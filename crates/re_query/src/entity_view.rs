use std::{collections::BTreeMap, marker::PhantomData};

use arrow2::array::{Array, MutableArray, PrimitiveArray};
use re_arrow_store::ArrayExt;
use re_format::arrow;
use re_log_types::{
    component_types::InstanceKey,
    external::arrow2_convert::{
        deserialize::{arrow_array_deserialize_iterator, ArrowArray, ArrowDeserialize},
        field::ArrowField,
        serialize::ArrowSerialize,
    },
    msg_bundle::Component,
    ComponentName,
};

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
    // TODO(jleibs): Remove optional once the store guarantees this will always exist
    pub(crate) instance_keys: Option<Box<dyn Array>>,
    pub(crate) values: Box<dyn Array>,
}

impl ComponentWithInstances {
    pub fn name(&self) -> ComponentName {
        self.name
    }

    /// Number of values. 1 for splats.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.len() == 0
    }

    /// Iterate over the instance keys
    ///
    /// If the instance keys don't exist, generate them based on array-index position of the values
    pub fn iter_instance_keys(&self) -> crate::Result<impl Iterator<Item = InstanceKey> + '_> {
        if let Some(keys) = &self.instance_keys {
            let iter = arrow_array_deserialize_iterator::<InstanceKey>(keys.as_ref())?;
            Ok(itertools::Either::Left(iter))
        } else {
            let auto_num = (0..self.len()).map(|i| InstanceKey(i as u64));
            Ok(itertools::Either::Right(auto_num))
        }
    }

    /// Iterate over the values and convert them to a native `Component`
    pub fn iter_values<C: Component>(&self) -> crate::Result<impl Iterator<Item = Option<C>> + '_>
    where
        C: ArrowDeserialize + ArrowField<Type = C> + 'static,
        C::ArrayType: ArrowArray,
        for<'a> &'a C::ArrayType: IntoIterator,
    {
        if C::name() != self.name {
            return Err(QueryError::TypeMismatch {
                actual: self.name,
                requested: C::name(),
            });
        }

        Ok(arrow_array_deserialize_iterator::<Option<C>>(
            self.values.as_ref(),
        )?)
    }

    /// Look up the value that corresponds to a given `InstanceKey` and convert to `Component`
    pub fn lookup<C: Component>(&self, instance_key: &InstanceKey) -> crate::Result<C>
    where
        C: ArrowDeserialize + ArrowField<Type = C> + 'static,
        C::ArrayType: ArrowArray,
        for<'a> &'a C::ArrayType: IntoIterator,
    {
        if C::name() != self.name {
            return Err(QueryError::TypeMismatch {
                actual: self.name,
                requested: C::name(),
            });
        }
        let arr = self
            .lookup_arrow(instance_key)
            .map_or_else(|| Err(QueryError::ComponentNotFound), Ok)?;
        let mut iter = arrow_array_deserialize_iterator::<Option<C>>(arr.as_ref())?;
        let val = iter
            .next()
            .flatten()
            .map_or_else(|| Err(QueryError::ComponentNotFound), Ok)?;
        Ok(val)
    }

    /// Look up the value that corresponds to a given `InstanceKey` and return as an arrow `Array`
    pub fn lookup_arrow(&self, instance_key: &InstanceKey) -> Option<Box<dyn Array>> {
        let offset = if let Some(instance_keys) = &self.instance_keys {
            // If `instance_keys` is set, extract the `PrimitiveArray`, and find
            // the index of the value by `binary_search`

            // The store should guarantee this for us but assert to be sure
            debug_assert!(instance_keys.is_sorted_and_unique().unwrap_or(false));

            let keys = instance_keys
                .as_any()
                .downcast_ref::<PrimitiveArray<u64>>()?
                .values();

            // If the value is splatted, return the offset of the splat
            if keys.len() == 1 && keys[0] == InstanceKey::SPLAT.0 {
                0
            } else {
                // Otherwise binary search to find the offset of the instance
                keys.binary_search(&instance_key.0).ok()?
            }
        } else {
            // If `instance_keys` is not set, then offset is the instance because the implicit
            // index is a sequential list
            let offset = instance_key.0 as usize;
            (offset < self.values.len()).then_some(offset)?
        };

        Some(self.values.slice(offset, 1))
    }

    /// Produce a `ComponentWithInstances` from native component types
    pub fn from_native<C>(
        instance_keys: Option<&Vec<InstanceKey>>,
        values: &Vec<C>,
    ) -> crate::Result<ComponentWithInstances>
    where
        C: Component + 'static,
        C: ArrowSerialize + ArrowField<Type = C>,
    {
        use re_log_types::external::arrow2_convert::serialize::arrow_serialize_to_mutable_array;

        let instance_keys = if let Some(keys) = instance_keys {
            Some(
                arrow_serialize_to_mutable_array::<InstanceKey, InstanceKey, &Vec<InstanceKey>>(
                    keys,
                )?
                .as_box(),
            )
        } else {
            None
        };

        let values = arrow_serialize_to_mutable_array::<C, C, &Vec<C>>(values)?.as_box();

        Ok(ComponentWithInstances {
            name: C::name(),
            instance_keys,
            values,
        })
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
pub struct EntityView<Primary: Component> {
    pub(crate) primary: ComponentWithInstances,
    pub(crate) components: BTreeMap<ComponentName, ComponentWithInstances>,
    pub(crate) phantom: PhantomData<Primary>,
}

impl<Primary: Component> std::fmt::Display for EntityView<Primary> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let primary_table = arrow::format_table(
            [
                self.primary.instance_keys.as_ref().unwrap().as_ref(),
                self.primary.values.as_ref(),
            ],
            ["InstanceId", self.primary.name.as_str()],
        );

        f.write_fmt(format_args!("EntityView:\n{primary_table}"))
    }
}

impl<Primary> EntityView<Primary>
where
    Primary: Component,
{
    #[inline]
    pub fn num_instances(&self) -> usize {
        self.primary.len()
    }
}

impl<Primary> EntityView<Primary>
where
    Primary: Component + ArrowSerialize + ArrowDeserialize + ArrowField<Type = Primary> + 'static,
    Primary::ArrayType: ArrowArray,
    for<'a> &'a Primary::ArrayType: IntoIterator,
{
    /// Iterate over the instance keys
    pub fn iter_instance_keys(&self) -> crate::Result<impl Iterator<Item = InstanceKey> + '_> {
        self.primary.iter_instance_keys()
    }

    /// Iterate over the primary component values.
    pub fn iter_primary(&self) -> crate::Result<impl Iterator<Item = Option<Primary>> + '_> {
        self.primary.iter_values()
    }

    /// Iterate over the flattened list of primary component values if any.
    pub fn iter_primary_flattened(&self) -> impl Iterator<Item = Primary> + '_ {
        self.primary
            .iter_values()
            .ok()
            .into_iter()
            .flatten()
            .flatten()
    }

    /// Check if the entity has a component and its not empty
    pub fn has_component<C: Component>(&self) -> bool {
        self.components
            .get(&C::name())
            .map_or(false, |c| !c.is_empty())
    }

    /// Iterate over the values of a `Component`.
    ///
    /// Always produces an iterator of length `self.primary.len()`
    pub fn iter_component<C: Component>(
        &self,
    ) -> crate::Result<impl Iterator<Item = Option<C>> + '_>
    where
        C: Clone + ArrowDeserialize + ArrowField<Type = C> + 'static,
        C::ArrayType: ArrowArray,
        for<'b> &'b C::ArrayType: IntoIterator,
    {
        let component = self.components.get(&C::name());

        if let Some(component) = component {
            let primary_instance_key_iter = self.primary.iter_instance_keys()?;

            let mut component_instance_key_iter = component.iter_instance_keys()?;

            let component_value_iter =
                arrow_array_deserialize_iterator::<Option<C>>(component.values.as_ref())?;

            let next_component_instance_key = component_instance_key_iter.next();

            Ok(itertools::Either::Left(ComponentJoinedIterator {
                primary_instance_key_iter,
                component_instance_key_iter,
                component_value_iter,
                next_component_instance_key,
                splatted_component_value: None,
            }))
        } else {
            let nulls = (0..self.primary.values.len()).map(|_| None);
            Ok(itertools::Either::Right(nulls))
        }
    }

    /// Helper function to produce an `EntityView` from rust-native `field_types`
    pub fn from_native(c0: (Option<&Vec<InstanceKey>>, &Vec<Primary>)) -> crate::Result<Self> {
        let primary = ComponentWithInstances::from_native(c0.0, c0.1)?;

        Ok(Self {
            primary,
            components: Default::default(),
            phantom: PhantomData,
        })
    }

    /// Helper function to produce an `EntityView` from rust-native `field_types`
    pub fn from_native2<C>(
        primary: (Option<&Vec<InstanceKey>>, &Vec<Primary>),
        component: (Option<&Vec<InstanceKey>>, &Vec<C>),
    ) -> crate::Result<Self>
    where
        C: Component + 'static,
        C: ArrowSerialize + ArrowField<Type = C>,
    {
        let primary = ComponentWithInstances::from_native(primary.0, primary.1)?;
        let component_c1 = ComponentWithInstances::from_native(component.0, component.1)?;

        let components = [(component_c1.name, component_c1)].into();

        Ok(Self {
            primary,
            components,
            phantom: PhantomData,
        })
    }
}

#[test]
fn lookup_value() {
    use re_log_types::component_types::{InstanceKey, Point2D, Rect2D};
    use re_log_types::external::arrow2_convert::serialize::arrow_serialize_to_mutable_array;
    let points = vec![
        Point2D { x: 1.0, y: 2.0 }, //
        Point2D { x: 3.0, y: 4.0 },
        Point2D { x: 5.0, y: 6.0 },
        Point2D { x: 7.0, y: 8.0 },
        Point2D { x: 9.0, y: 10.0 },
    ];

    let component = ComponentWithInstances::from_native(None, &points).unwrap();

    let missing_value = component.lookup_arrow(&InstanceKey(5));
    assert_eq!(missing_value, None);

    let value = component.lookup_arrow(&InstanceKey(2)).unwrap();

    let expected_point = vec![points[2].clone()];
    let expected_arrow =
        arrow_serialize_to_mutable_array::<Point2D, Point2D, &Vec<Point2D>>(&expected_point)
            .unwrap()
            .as_box();

    assert_eq!(expected_arrow, value);

    let instance_keys = vec![
        InstanceKey(17),
        InstanceKey(47),
        InstanceKey(48),
        InstanceKey(99),
        InstanceKey(472),
    ];

    let component = ComponentWithInstances::from_native(Some(&instance_keys), &points).unwrap();

    let missing_value = component.lookup_arrow(&InstanceKey(46));
    assert_eq!(missing_value, None);

    let value = component.lookup_arrow(&InstanceKey(99)).unwrap();

    let expected_point = vec![points[3].clone()];
    let expected_arrow =
        arrow_serialize_to_mutable_array::<Point2D, Point2D, &Vec<Point2D>>(&expected_point)
            .unwrap()
            .as_box();

    assert_eq!(expected_arrow, value);

    // Lookups with serialization

    let value = component.lookup::<Point2D>(&InstanceKey(99)).unwrap();
    assert_eq!(expected_point[0], value);

    let missing_value = component.lookup::<Point2D>(&InstanceKey(46));
    assert!(matches!(
        missing_value.err().unwrap(),
        QueryError::ComponentNotFound
    ));

    let missing_value = component.lookup::<Rect2D>(&InstanceKey(99));
    assert!(matches!(
        missing_value.err().unwrap(),
        QueryError::TypeMismatch { .. }
    ));
}

#[test]
fn lookup_splat() {
    use re_log_types::component_types::{InstanceKey, Point2D};
    let instances = vec![
        InstanceKey::SPLAT, //
    ];
    let points = vec![
        Point2D { x: 1.0, y: 2.0 }, //
    ];

    let component = ComponentWithInstances::from_native(Some(&instances), &points).unwrap();

    // Any instance we look up will return the slatted value
    let value = component.lookup::<Point2D>(&InstanceKey(1)).unwrap();
    assert_eq!(points[0], value);

    let value = component.lookup::<Point2D>(&InstanceKey(99)).unwrap();
    assert_eq!(points[0], value);
}
