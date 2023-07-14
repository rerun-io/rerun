use std::{borrow::Borrow, collections::BTreeMap, marker::PhantomData};

use arrow2::array::{Array, PrimitiveArray};
use itertools::{Either, Itertools};
use re_format::arrow;
use re_log_types::RowId;
use re_types::{components::InstanceKey, Archetype, Component, ComponentName, Loggable};

use crate::QueryError;

/// A type-erased array of [`Component`] values and the corresponding [`InstanceKey`] keys.
///
/// `instance_keys` must always be sorted if present. If not present we assume implicit
/// instance keys that are equal to the row-number.
///
/// See: [`crate::get_component_with_instances`]
#[derive(Clone, Debug)]
pub struct ComponentWithInstances {
    pub(crate) name: ComponentName,
    pub(crate) instance_keys: Box<dyn ::arrow2::array::Array>,
    pub(crate) values: Box<dyn ::arrow2::array::Array>,
}

impl ComponentWithInstances {
    #[inline]
    pub fn name(&self) -> &ComponentName {
        &self.name
    }

    /// Number of values. 1 for splats.
    #[inline]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Iterate over the instance keys
    ///
    /// If the instance keys don't exist, generate them based on array-index position of the values
    #[inline]
    pub fn iter_instance_keys(&self) -> impl Iterator<Item = InstanceKey> + '_ {
        InstanceKey::from_arrow(self.instance_keys.as_ref()).into_iter()
    }

    /// Iterate over the values and convert them to a native `Component`
    // TODO(jleibs): These bounds seem weird
    #[inline]
    pub fn iter_values<'a, C: Component + 'a>(
        &self,
    ) -> crate::Result<impl Iterator<Item = Option<C>> + 'a> {
        if &C::name() != self.name() {
            return Err(QueryError::NewTypeMismatch {
                actual: self.name().clone(),
                requested: C::name(),
            });
        }

        Ok(C::try_from_arrow_opt(self.values.as_ref())?.into_iter())
    }

    /// Look up the value that corresponds to a given `InstanceKey` and convert to `Component`
    pub fn lookup<C: Component>(&self, instance_key: &InstanceKey) -> crate::Result<C> {
        if &C::name() != self.name() {
            return Err(QueryError::NewTypeMismatch {
                actual: self.name().clone(),
                requested: C::name(),
            });
        }
        let arr = self
            .lookup_arrow(instance_key)
            .map_or_else(|| Err(QueryError::ComponentNotFound), Ok)?;

        let mut iter = C::try_from_arrow(arr.as_ref())?.into_iter();

        let val = iter
            .next()
            .map_or_else(|| Err(QueryError::ComponentNotFound), Ok)?;
        Ok(val)
    }

    /// Look up the value that corresponds to a given `InstanceKey` and return as an arrow `Array`
    pub fn lookup_arrow(&self, instance_key: &InstanceKey) -> Option<Box<dyn Array>> {
        let keys = self
            .instance_keys
            .as_any()
            .downcast_ref::<PrimitiveArray<u64>>()?
            .values();

        // If the value is splatted, return the offset of the splat
        let offset = if keys.len() == 1 && keys[0] == InstanceKey::SPLAT.0 {
            0
        } else {
            // Otherwise binary search to find the offset of the instance
            keys.binary_search(&instance_key.0).ok()?
        };

        (self.values.len() > offset)
            .then(|| self.values.sliced(offset, 1))
            .or_else(|| {
                re_log::error_once!("found corrupt cell -- mismatched number of instances");
                None
            })
    }

    /// Produce a `ComponentWithInstances` from native component types
    pub fn from_native<'a, C: Component + Clone + 'a>(
        instance_keys: impl IntoIterator<Item = impl Into<::std::borrow::Cow<'a, InstanceKey>>>,
        values: impl IntoIterator<Item = impl Into<::std::borrow::Cow<'a, C>>>,
    ) -> ComponentWithInstances {
        let instance_keys = InstanceKey::to_arrow(instance_keys, None);
        let values = C::to_arrow(values, None);
        ComponentWithInstances {
            name: C::name(),
            instance_keys,
            values,
        }
    }
}

/// Iterator over a single component joined onto a primary component
///
/// This is equivalent to a left join between one table made up of the
/// instance-keys from the primary component and another table with the
/// instance-keys and values of the iterated component.
///
/// Instances have a special "splat" key that will cause the value to be
/// repeated for the entirety of the join.
///
/// For example
/// ```text
/// primary
/// +----------+
/// | instance |
/// +----------+
/// | key0     |
/// | key1     |
/// | Key2     |
///
/// component
/// +----------+-------+
/// | instance | value |
/// +----------+-------+
/// | key0     | val0  |
/// | Key2     | val2  |
///
/// SELECT value FROM LEFT JOIN primary.instance = component.instance;
///
/// output
/// +-------+
/// | value |
/// +-------+
/// | val0  |
/// | NULL  |
/// | val2  |
///
/// ```
struct ComponentJoinedIterator<IIter1, IIter2, VIter, Val> {
    primary_instance_key_iter: IIter1,
    component_instance_key_iter: IIter2,
    component_value_iter: VIter,
    next_component_instance_key: Option<InstanceKey>,
    splatted_component_value: Option<Val>,
}

impl<IIter1, IIter2, VIter, C> Iterator for ComponentJoinedIterator<IIter1, IIter2, VIter, C>
where
    IIter1: Iterator<Item = InstanceKey>,
    IIter2: Iterator<Item = InstanceKey>,
    VIter: Iterator<Item = Option<C>>,
    C: Clone,
{
    type Item = Option<C>;

    fn next(&mut self) -> Option<Option<C>> {
        // For each value of primary_instance_iter we must find a result
        if let Some(primary_key) = self.primary_instance_key_iter.next() {
            loop {
                match &self.next_component_instance_key {
                    // If we have a next component key, we either...
                    Some(instance_key) => {
                        if instance_key.is_splat() {
                            if self.splatted_component_value.is_none() {
                                self.splatted_component_value =
                                    self.component_value_iter.next().flatten();
                            }
                            break Some(self.splatted_component_value.clone());
                        } else {
                            match primary_key.0.cmp(&instance_key.0) {
                                // Return a None if the primary_key hasn't reached it yet
                                std::cmp::Ordering::Less => break Some(None),
                                // Return the value if the keys match
                                std::cmp::Ordering::Equal => {
                                    self.next_component_instance_key =
                                        self.component_instance_key_iter.next();
                                    break self.component_value_iter.next();
                                }
                                // Skip this component if the key is behind the primary key
                                std::cmp::Ordering::Greater => {
                                    _ = self.component_value_iter.next();
                                    self.next_component_instance_key =
                                        self.component_instance_key_iter.next();
                                }
                            }
                        }
                    }
                    // Otherwise, we ran out of component elements. Just return
                    // None until the primary iter ends.
                    None => break Some(None),
                };
            }
        } else {
            None
        }
    }
}

/// A view of an entity at a particular point in time returned by [`crate::get_component_with_instances`]
///
/// `EntityView` has a special `primary` [`Component`] which determines the length of an entity
/// batch. When iterating over individual components, they will be implicitly joined onto
/// the primary component using instance keys.
#[derive(Clone, Debug)]
pub struct ArchetypeView<A: Archetype> {
    pub(crate) row_id: RowId,
    pub(crate) components: BTreeMap<ComponentName, ComponentWithInstances>,
    pub(crate) phantom: PhantomData<A>,
}

impl<A: Archetype> std::fmt::Display for ArchetypeView<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        /*
        let primary_table = arrow::format_table(
            [
                self.primary.instance_keys.as_arrow_ref(),
                self.primary.values.as_arrow_ref(),
            ],
            ["InstanceId", self.primary.name().as_str()],
        );
        */

        //f.write_fmt(format_args!("ArchetypeView:\n{primary_table}"))
        let name = A::name();
        f.write_fmt(format_args!("ArchetypeView:\n{name}"))
    }
}

impl<A: Archetype> ArchetypeView<A> {
    #[inline]
    pub fn num_instances(&self) -> usize {
        self.primary_comp().len()
    }

    #[inline]
    pub fn row_id(&self) -> RowId {
        self.row_id
    }
}

impl<A: Archetype> ArchetypeView<A> {
    fn primary_comp(&self) -> &ComponentWithInstances {
        // TODO(jleibs): Do all archetypes always have at least 1 required components?
        let primary_name = A::required_components().get(0).unwrap().clone();
        self.components.get(&primary_name).unwrap()
    }

    /// Iterate over the instance keys
    #[inline]
    pub fn iter_instance_keys(&self) -> impl Iterator<Item = InstanceKey> + '_ {
        self.primary_comp().iter_instance_keys()
    }

    /// Check if the entity has a component and its not empty
    #[inline]
    pub fn has_component<C: Component>(&self) -> bool {
        let name = C::name();
        self.components.get(&name).map_or(false, |c| !c.is_empty())
    }

    /// Iterate over the values of a `Component`.
    ///
    /// Always produces an iterator of length `self.primary.len()`
    pub fn iter_component<'a, C: Component + Clone + 'a>(
        &'a self,
    ) -> crate::Result<impl Iterator<Item = Option<C>> + '_> {
        let component = self.components.get(&C::name());

        if let Some(component) = component {
            let primary_instance_key_iter = self.iter_instance_keys();

            let mut component_instance_key_iter = component.iter_instance_keys();

            let component_value_iter =
                C::try_from_arrow_opt(component.values.as_ref())?.into_iter();

            let next_component_instance_key = component_instance_key_iter.next();

            Ok(itertools::Either::Left(ComponentJoinedIterator {
                primary_instance_key_iter,
                component_instance_key_iter,
                component_value_iter,
                next_component_instance_key,
                splatted_component_value: None,
            }))
        } else {
            let primary = self.primary_comp();
            let nulls = (0..primary.len()).map(|_| None);
            Ok(itertools::Either::Right(nulls))
        }
    }

    /// Helper function to produce an `EntityView` from rust-native `field_types`
    #[inline]
    pub fn from_native(arch: A) -> Self {
        Self {
            row_id: RowId::ZERO,
            components: Default::default(),
            phantom: PhantomData,
        }
    }
}

#[test]
fn lookup_value() {
    use re_types::components::{InstanceKey, Point2D};

    let instance_keys = (0..5).map(InstanceKey).collect_vec();

    let points = [
        Point2D::new(1.0, 2.0), //
        Point2D::new(3.0, 4.0),
        Point2D::new(5.0, 6.0),
        Point2D::new(7.0, 8.0),
        Point2D::new(9.0, 10.0),
    ];

    let component = ComponentWithInstances::from_native(instance_keys, points);

    let missing_value = component.lookup_arrow(&InstanceKey(5));
    assert_eq!(missing_value, None);

    let value = component.lookup_arrow(&InstanceKey(2)).unwrap();

    let expected_point = [points[2].clone()];
    let expected_arrow = Point2D::to_arrow(expected_point, None);

    assert_eq!(expected_arrow, value);

    let instance_keys = [
        InstanceKey(17),
        InstanceKey(47),
        InstanceKey(48),
        InstanceKey(99),
        InstanceKey(472),
    ];

    let component = ComponentWithInstances::from_native(instance_keys, points);

    let missing_value = component.lookup_arrow(&InstanceKey(46));
    assert_eq!(missing_value, None);

    let value = component.lookup_arrow(&InstanceKey(99)).unwrap();

    let expected_point = [points[3].clone()];
    let expected_arrow = Point2D::to_arrow(expected_point, None);

    assert_eq!(expected_arrow, value);

    // Lookups with serialization

    let value = component.lookup::<Point2D>(&InstanceKey(99)).unwrap();
    assert_eq!(expected_point[0], value);

    let missing_value = component.lookup::<Point2D>(&InstanceKey(46));
    assert!(matches!(
        missing_value.err().unwrap(),
        QueryError::ComponentNotFound
    ));

    // TODO(jleibs): Add another type
    /*
    let missing_value = component.lookup::<Rect2D>(&InstanceKey(99));
    assert!(matches!(
        missing_value.err().unwrap(),
        QueryError::TypeMismatch { .. }
    ));
    */
}

#[test]
fn lookup_splat() {
    use re_types::components::{InstanceKey, Point2D};
    let instances = [
        InstanceKey::SPLAT, //
    ];
    let points = [
        Point2D::new(1.0, 2.0), //
    ];

    let component = ComponentWithInstances::from_native(instances, points);

    // Any instance we look up will return the slatted value
    let value = component.lookup::<Point2D>(&InstanceKey(1)).unwrap();
    assert_eq!(points[0], value);

    let value = component.lookup::<Point2D>(&InstanceKey(99)).unwrap();
    assert_eq!(points[0], value);
}
