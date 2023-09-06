use std::{collections::BTreeMap, marker::PhantomData};

use re_format::arrow;
use re_log_types::RowId;
use re_types::{components::InstanceKey, Component, ComponentName};

use crate::{archetype_view::ComponentJoinedIterator, ComponentWithInstances};

/// A view of an entity at a particular point in time returned by [`crate::get_component_with_instances`]
///
/// `EntityView` has a special `primary` [`Component`] which determines the length of an entity
/// batch. When iterating over individual components, they will be implicitly joined onto
/// the primary component using instance keys.
#[derive(Clone, Debug)]
pub struct EntityView<Primary: Component> {
    pub(crate) primary_row_id: RowId,
    pub(crate) primary: ComponentWithInstances,
    pub(crate) components: BTreeMap<ComponentName, ComponentWithInstances>,
    pub(crate) phantom: PhantomData<Primary>,
}

impl<Primary: Component> std::fmt::Display for EntityView<Primary> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let primary_table = arrow::format_table(
            [
                self.primary.instance_keys.as_arrow_ref(),
                self.primary.values.as_arrow_ref(),
            ],
            ["InstanceId", self.primary.name().as_ref()],
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

    /// Returns the [`RowId`] associated with the _primary_ component that was used to drive this
    /// entire query.
    ///
    /// Beware: when using this [`RowId`] for caching/versioning purposes, make sure the component
    /// you are about to cache is in fact the primary component of the query!
    /// See also <https://github.com/rerun-io/rerun/issues/3232>.
    #[inline]
    pub fn primary_row_id(&self) -> RowId {
        self.primary_row_id
    }
}

impl<'a, Primary> EntityView<Primary>
where
    Primary: Component + 'a,
    &'a Primary: Into<::std::borrow::Cow<'a, Primary>>,
{
    /// Iterate over the instance keys
    #[inline]
    pub fn iter_instance_keys(&self) -> impl Iterator<Item = InstanceKey> + '_ {
        self.primary.instance_keys().into_iter()
    }

    /// Iterate over the primary component values.
    #[inline]
    pub fn iter_primary(&self) -> crate::Result<impl Iterator<Item = Option<Primary>> + '_> {
        Ok(self.primary.values()?.into_iter())
    }

    /// Iterate over the flattened list of primary component values if any.
    #[inline]
    pub fn iter_primary_flattened(&self) -> impl Iterator<Item = Primary> + '_ {
        self.primary.values().ok().into_iter().flatten().flatten()
    }

    /// Check if the entity has a component and its not empty
    #[inline]
    pub fn has_component<C: Component>(&self) -> bool {
        self.components
            .get(&C::name())
            .map_or(false, |c| !c.is_empty())
    }

    /// Iterate over the values of a `Component`.
    ///
    /// Always produces an iterator of length `self.primary.len()`
    pub fn iter_component<'b, C: Component + 'b>(
        &'b self,
    ) -> crate::Result<impl Iterator<Item = Option<C>> + '_> {
        let component = self.components.get(&C::name());

        if let Some(component) = component {
            let primary_instance_key_iter = self.primary.instance_keys().into_iter();

            let mut component_instance_key_iter = component.instance_keys().into_iter();

            let component_value_iter = component.values.try_to_native_opt()?.into_iter();

            let next_component_instance_key = component_instance_key_iter.next();

            Ok(itertools::Either::Left(ComponentJoinedIterator {
                primary_instance_key_iter,
                component_instance_key_iter,
                component_value_iter,
                next_component_instance_key,
                splatted_component_value: None,
            }))
        } else {
            let nulls = (0..self.primary.values.num_instances()).map(|_| None);
            Ok(itertools::Either::Right(nulls))
        }
    }

    /// Helper function to produce an `EntityView` from rust-native `field_types`
    #[inline]
    pub fn from_native(c0: (&'a [InstanceKey], &'a [Primary])) -> Self where {
        // Need to convert to new-style keys
        let primary = ComponentWithInstances::from_native(c0.0, c0.1);
        Self {
            primary_row_id: RowId::ZERO,
            primary,
            components: Default::default(),
            phantom: PhantomData,
        }
    }

    /// Helper function to produce an `EntityView` from rust-native `field_types`
    #[inline]
    pub fn from_native2<C>(
        primary: (&'a [InstanceKey], &'a [Primary]),
        component: (&'a [InstanceKey], &'a [C]),
    ) -> Self
    where
        C: Component + 'a,
        &'a C: Into<::std::borrow::Cow<'a, C>>,
    {
        let primary = ComponentWithInstances::from_native::<Primary>(primary.0, primary.1);
        let component_c1 = ComponentWithInstances::from_native::<C>(component.0, component.1);

        let components = [(component_c1.name(), component_c1)].into();

        Self {
            primary_row_id: RowId::ZERO,
            primary,
            components,
            phantom: PhantomData,
        }
    }
}

#[test]
fn lookup_value() {
    use crate::QueryError;
    use re_components::Rect2D;
    use re_types::components::Point2D;
    use re_types::Loggable as _;

    let instance_keys = InstanceKey::from_iter(0..5);

    let points = [
        Point2D::new(1.0, 2.0), //
        Point2D::new(3.0, 4.0),
        Point2D::new(5.0, 6.0),
        Point2D::new(7.0, 8.0),
        Point2D::new(9.0, 10.0),
    ];

    let component =
        ComponentWithInstances::from_native(instance_keys.as_slice(), points.as_slice());

    let missing_value = component.lookup_arrow(&InstanceKey(5));
    assert_eq!(missing_value, None);

    let value = component.lookup_arrow(&InstanceKey(2)).unwrap();

    let expected_point = [points[2]];
    let expected_arrow = Point2D::to_arrow(expected_point);

    assert_eq!(expected_arrow, value);

    let instance_keys = [
        InstanceKey(17),
        InstanceKey(47),
        InstanceKey(48),
        InstanceKey(99),
        InstanceKey(472),
    ];

    let component = ComponentWithInstances::from_native(instance_keys.as_slice(), points);

    let missing_value = component.lookup_arrow(&InstanceKey(46));
    assert_eq!(missing_value, None);

    let value = component.lookup_arrow(&InstanceKey(99)).unwrap();

    let expected_point = [points[3]];
    let expected_arrow = Point2D::to_arrow(expected_point);

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
    use re_types::components::Point2D;
    let instances = [
        InstanceKey::SPLAT, //
    ];
    let points = [Point2D::new(1.0, 2.0)];

    let component = ComponentWithInstances::from_native(instances.as_slice(), points.as_slice());

    // Any instance we look up will return the slatted value
    let value = component.lookup::<Point2D>(&InstanceKey(1)).unwrap();
    assert_eq!(points[0], value);

    let value = component.lookup::<Point2D>(&InstanceKey(99)).unwrap();
    assert_eq!(points[0], value);
}
