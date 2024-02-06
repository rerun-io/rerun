use std::{collections::BTreeMap, marker::PhantomData};

use arrow2::array::{Array, PrimitiveArray};
use re_format::arrow;
use re_log_types::{DataCell, DataCellRow, RowId, TimeInt};
use re_types_core::{
    components::InstanceKey, Archetype, Component, ComponentName, DeserializationError,
    DeserializationResult, Loggable, SerializationResult,
};

use crate::QueryError;

/// A type-erased array of [`Component`] values and the corresponding [`InstanceKey`] keys.
///
/// See: [`crate::get_component_with_instances`]
#[derive(Clone, Debug)]
pub struct ComponentWithInstances {
    pub(crate) instance_keys: DataCell,
    pub(crate) values: DataCell,
}

impl std::fmt::Display for ComponentWithInstances {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let table = arrow::format_table(
            [
                self.instance_keys.as_arrow_ref(),
                self.values.as_arrow_ref(),
            ],
            ["InstanceKey", self.values.component_name().as_ref()],
        );

        f.write_fmt(format_args!("ComponentWithInstances:\n{table}"))
    }
}

impl ComponentWithInstances {
    /// Name of the [`Component`]
    #[inline]
    pub fn name(&self) -> ComponentName {
        self.values.component_name()
    }

    /// Number of values. 1 for splats.
    #[inline]
    pub fn len(&self) -> usize {
        self.values.num_instances() as _
    }

    #[inline]
    /// Whether this [`ComponentWithInstances`] contains any data
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Returns the array of [`InstanceKey`]s.
    #[inline]
    pub fn instance_keys(&self) -> Vec<InstanceKey> {
        re_tracing::profile_function!();
        self.instance_keys.to_native::<InstanceKey>()
    }

    /// Returns the array of values as native [`Component`]s.
    #[inline]
    pub fn values<'a, C: Component + 'a>(&'a self) -> crate::Result<Vec<Option<C>>> {
        if C::name() != self.name() {
            return Err(QueryError::TypeMismatch {
                actual: self.name(),
                requested: C::name(),
            });
        }

        Ok(self.values.try_to_native_opt::<'a, C>()?)
    }

    /// Look up the value that corresponds to a given [`InstanceKey`] and convert to [`Component`]
    pub fn lookup<C: Component>(&self, instance_key: &InstanceKey) -> crate::Result<C> {
        if C::name() != self.name() {
            return Err(QueryError::TypeMismatch {
                actual: self.name(),
                requested: C::name(),
            });
        }
        let arr = self
            .lookup_arrow(instance_key)
            .map_or_else(|| Err(crate::ComponentNotFoundError(C::name())), Ok)?;

        let mut iter = C::from_arrow(arr.as_ref())?.into_iter();

        let val = iter
            .next()
            .map_or_else(|| Err(crate::ComponentNotFoundError(C::name())), Ok)?;
        Ok(val)
    }

    /// Look up the value that corresponds to a given [`InstanceKey`] and return as an arrow [`Array`]
    pub fn lookup_arrow(&self, instance_key: &InstanceKey) -> Option<Box<dyn Array>> {
        let keys = self
            .instance_keys
            .as_arrow_ref()
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

        (self.len() > offset)
            .then(|| self.values.as_arrow_ref().sliced(offset, 1))
            .or_else(|| {
                re_log::error_once!("found corrupt cell -- mismatched number of instances");
                None
            })
    }

    /// Produce a [`ComponentWithInstances`] from native [`Component`] types
    pub fn from_native<'a, C: Component + Clone + 'a>(
        instance_keys: impl IntoIterator<Item = impl Into<::std::borrow::Cow<'a, InstanceKey>>>,
        values: impl IntoIterator<Item = impl Into<::std::borrow::Cow<'a, C>>>,
    ) -> SerializationResult<ComponentWithInstances> {
        // Unwrap: If the data is valid for the native types, it's valid in serialized form.
        let instance_keys = InstanceKey::to_arrow(instance_keys)?;
        let values = C::to_arrow(values)?;
        Ok(ComponentWithInstances {
            instance_keys: DataCell::from_arrow(InstanceKey::name(), instance_keys),
            values: DataCell::from_arrow(C::name(), values),
        })
    }

    #[inline]
    pub fn into_data_cell_row(self) -> DataCellRow {
        let Self {
            instance_keys,
            values,
        } = self;
        DataCellRow(smallvec::smallvec![instance_keys, values])
    }
}

/// Iterator over a single [`Component`] joined onto a primary [`Component`]
///
/// This is equivalent to a left join between one table made up of the
/// [`InstanceKey`]s from the primary component and another table with the
/// [`InstanceKey`]s and values of the iterated [`Component`].
///
/// Instances have a [`InstanceKey::SPLAT`] key that will cause the value to be
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
pub struct ComponentJoinedIterator<IIter1, IIter2, VIter, Val> {
    pub primary_instance_key_iter: IIter1,
    pub component_instance_key_iter: IIter2,
    pub component_value_iter: VIter,
    pub next_component_instance_key: Option<InstanceKey>,
    pub splatted_component_value: Option<Val>,
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
                    // If we have a next component key, we either…
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

impl<IIter1, IIter2, VIter, C> ExactSizeIterator
    for ComponentJoinedIterator<IIter1, IIter2, VIter, C>
where
    IIter1: ExactSizeIterator<Item = InstanceKey>,
    IIter2: ExactSizeIterator<Item = InstanceKey>,
    VIter: ExactSizeIterator<Item = Option<C>>,
    C: Clone,
{
}

/// A view of an [`Archetype`] at a particular point in time returned by [`crate::get_component_with_instances`].
///
/// The required [`Component`]s of an [`ArchetypeView`] determines the length of an entity
/// batch. When iterating over individual components, they will be implicitly joined onto
/// the required [`Component`]s using [`InstanceKey`] values.
#[derive(Clone, Debug)]
pub struct ArchetypeView<A: Archetype> {
    /// The _data_ time of the most recent component in the view (not necessarily the primary!).
    ///
    /// `None` if timeless.
    pub(crate) data_time: Option<TimeInt>,

    /// The [`RowId`] of the primary component in the view.
    pub(crate) primary_row_id: RowId,

    pub(crate) components: BTreeMap<ComponentName, ComponentWithInstances>,

    pub(crate) phantom: PhantomData<A>,
}

impl<A: Archetype> std::fmt::Display for ArchetypeView<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let first_required = self.required_comp();

        let primary_table = arrow::format_table(
            [
                first_required.instance_keys.as_arrow_ref(),
                first_required.values.as_arrow_ref(),
            ],
            ["InstanceId", first_required.name().as_ref()],
        );

        f.write_fmt(format_args!("ArchetypeView:\n{primary_table}"))
    }
}

impl<A: Archetype> ArchetypeView<A> {
    #[inline]
    pub fn num_instances(&self) -> usize {
        self.required_comp().len()
    }

    /// The _data_ time of the most recent component in the view (not necessarily the primary!).
    ///
    /// `None` if timeless.
    #[inline]
    pub fn data_time(&self) -> Option<TimeInt> {
        self.data_time
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

impl<A: Archetype> ArchetypeView<A> {
    #[inline]
    fn required_comp(&self) -> &ComponentWithInstances {
        // TODO(jleibs): Do all archetypes always have at least 1 required components?
        let first_required = A::required_components()[0];
        &self.components[&first_required]
    }

    /// Returns an iterator over [`InstanceKey`]s.
    #[inline]
    pub fn iter_instance_keys(&self) -> impl ExactSizeIterator<Item = InstanceKey> {
        re_tracing::profile_function!();
        // TODO(#2750): Maybe make this an intersection instead
        self.required_comp().instance_keys().into_iter()
    }

    /// Check if the entity has a component and its not empty
    #[inline]
    pub fn has_component<C: Component>(&self) -> bool {
        let name = C::name();
        self.components.get(&name).map_or(false, |c| !c.is_empty())
    }

    /// Iterate over the values of a required multi-component.
    #[inline]
    pub fn iter_required_component<'a, C: Component + 'a>(
        &'a self,
    ) -> DeserializationResult<impl ExactSizeIterator<Item = C> + '_> {
        // NOTE(#3850): Don't add a profile scope here: the profiler overhead is too big for this fast function.
        // re_tracing::profile_function!();

        debug_assert!(A::required_components()
            .iter()
            .any(|c| c.as_ref() == C::name()));
        let component = self.components.get(&C::name());

        if let Some(component) = component {
            let component_value_iter = component
                .values
                .try_to_native()
                .map_err(|err| DeserializationError::DataCellError(err.to_string()))?
                .into_iter();

            Ok(component_value_iter)
        } else {
            Err(DeserializationError::MissingComponent {
                component: C::name(),
                backtrace: ::backtrace::Backtrace::new_unresolved(),
            })
        }
    }

    /// Get a single required mono-component.
    #[inline]
    pub fn required_mono_component<C: Component>(&self) -> DeserializationResult<C> {
        // NOTE(#3850): Don't add a profile scope here: the profiler overhead is too big for this fast function.
        // re_tracing::profile_function!();

        let mut iter = self.iter_required_component::<C>()?;
        let value = iter
            .next()
            .ok_or_else(|| DeserializationError::MissingComponent {
                component: C::name(),
                backtrace: re_types_core::_Backtrace::new_unresolved(),
            })?;
        let count = 1 + iter.count();
        if count != 1 {
            re_log::warn_once!("Expected a single value of {} but found {count}", C::name());
        }
        Ok(value)
    }

    /// Iterate over optional values as native [`Component`]s.
    ///
    /// Always produces an iterator that matches the length of a primary
    /// component by joining on the `InstanceKey` values.
    #[inline]
    pub fn iter_optional_component<'a, C: Component + Clone + 'a>(
        &'a self,
    ) -> DeserializationResult<impl ExactSizeIterator<Item = Option<C>> + '_> {
        // NOTE(#3850): Don't add a profile scope here: the profiler overhead is too big for this fast function.
        // re_tracing::profile_function!(C::name());

        let component = self.components.get(&C::name());

        // If the component is found and not empty, run the joining iterator on it.
        // Otherwise just output nulls of the length of the primary.
        // Note that this guard is specifically a precondition of the inner optimization
        // for matched instance keys which will debug_assert if a zero-length component is
        // referenced there.
        let is_empty = component.map_or(true, |c| c.is_empty());
        if let (Some(component), false) = (component, is_empty) {
            // NOTE(1): Autogenerated instance keys are interned behind datacells.
            // If two or more rows in the store share the same keys, then they will share
            // also the same cells.
            // Therefore we can compare those cells, and early out if they match.
            //
            // NOTE(2): Comparing cells that point to the same backing storage is a simple pointer
            // comparison; no data comparison involved.
            // If the cells are not interned, this will fall back to a more costly data comparison:
            // - If the data is the same, the cost of the comparison will be won back by having a
            //   faster iterator.
            // - If the data isn't the same, the cost of the comparison will be dwarfed by the cost
            //   of the join anyway.

            if self.required_comp().instance_keys == component.instance_keys {
                // This fast iterator is assumed to match the length of the
                // primary component We shouldn't hit this since the store
                // should enforce matched lengths for non-empty components, and
                // the outer if-guard should keep us from reaching this in the
                // case of an empty component.
                // TODO(#1893):  This assert and the implementation both need to
                // be addressed when we allow single rows containing splats.
                debug_assert!(
                    self.required_comp().instance_keys.num_instances()
                        == component.values.num_instances()
                );

                // NOTE: A component instance cannot be optional in itself, and if we're on this
                // path then we know for a fact that both batches can be intersected 1-to-1.
                // Therefore there cannot be any null values, therefore we can go through the fast
                // deserialization path.
                let component_value_iter = {
                    C::from_arrow(component.values.as_arrow_ref())?
                        .into_iter()
                        .map(Some)
                };

                return Ok(itertools::Either::Left(itertools::Either::Left(
                    component_value_iter,
                )));
            }

            let component_value_iter =
                { C::from_arrow_opt(component.values.as_arrow_ref())?.into_iter() };

            let primary_instance_key_iter = self.iter_instance_keys();
            let mut component_instance_key_iter = component.instance_keys().into_iter();

            let next_component_instance_key = component_instance_key_iter.next();

            Ok(itertools::Either::Left(itertools::Either::Right(
                ComponentJoinedIterator {
                    primary_instance_key_iter,
                    component_instance_key_iter,
                    component_value_iter,
                    next_component_instance_key,
                    splatted_component_value: None,
                },
            )))
        } else {
            let primary = self.required_comp();
            let nulls = (0..primary.len()).map(|_| None);
            Ok(itertools::Either::Right(nulls))
        }
    }

    /// Get a single optional mono-component.
    #[inline]
    pub fn optional_mono_component<C: Component>(&self) -> DeserializationResult<Option<C>> {
        let mut iter = self.iter_optional_component::<C>()?;
        if let Some(first_value) = iter.next() {
            let count = 1 + iter.count();
            if count != 1 {
                re_log::warn_once!("Expected a single value of {} but found {count}", C::name());
            }
            Ok(first_value)
        } else {
            Ok(None)
        }
    }

    /// Iterate over optional values as native [`Component`]s.
    ///
    /// The contents of the cell are returned as-is, without joining with any other component.
    #[inline]
    pub fn iter_raw_optional_component<'a, C: Component + Clone + 'a>(
        &'a self,
    ) -> DeserializationResult<Option<impl Iterator<Item = C> + '_>> {
        // NOTE(#3850): Don't add a profile scope here: the profiler overhead is too big for this fast function.
        // re_tracing::profile_function!(C::name());

        let component = self.components.get(&C::name());

        if let Some(component) = component {
            return Ok(Some(
                C::from_arrow(component.values.as_arrow_ref())?.into_iter(),
            ));
        }

        Ok(None)
    }

    /// Get a single optional mono-component.
    #[inline]
    pub fn raw_optional_mono_component<C: Component>(&self) -> DeserializationResult<Option<C>> {
        if let Some(mut iter) = self.iter_raw_optional_component::<C>()? {
            if let Some(value) = iter.next() {
                let count = 1 + iter.count();
                if count != 1 {
                    re_log::warn_once!(
                        "Expected a single value of {} but found {count}",
                        C::name()
                    );
                }
                Ok(Some(value))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Helper function to produce an [`ArchetypeView`] from a collection of [`ComponentWithInstances`].
    #[inline]
    pub fn from_components(
        data_time: Option<TimeInt>,
        primary_row_id: RowId,
        components: impl IntoIterator<Item = ComponentWithInstances>,
    ) -> Self {
        Self {
            data_time,
            primary_row_id,
            components: components
                .into_iter()
                .map(|comp| (comp.name(), comp))
                .collect(),
            phantom: PhantomData,
        }
    }

    /// Convert an `ArchetypeView` back into a native Archetype instance
    pub fn to_archetype(&self) -> crate::Result<A> {
        for component in A::required_components().iter() {
            if self
                .components
                .get(component)
                .map_or(true, |cwi| cwi.is_empty())
            {
                return Err(QueryError::PrimaryNotFound(*component));
            }
        }

        Ok(A::from_arrow_components(
            self.components
                .values()
                .map(|comp| (comp.name(), comp.values.to_arrow())),
        )?)
    }

    /// Useful for tests.
    pub fn to_data_cell_row_1<
        'a,
        C1: re_types_core::Component + Clone + Into<::std::borrow::Cow<'a, C1>> + 'a,
    >(
        &self,
    ) -> crate::Result<DataCellRow> {
        let cell0 = DataCell::from_native(self.iter_instance_keys());
        let cell1 = DataCell::from_native_sparse(self.iter_optional_component::<C1>()?);
        Ok(DataCellRow(smallvec::smallvec![cell0, cell1]))
    }

    /// Useful for tests.
    pub fn to_data_cell_row_2<
        'a,
        C1: re_types_core::Component + Clone + Into<::std::borrow::Cow<'a, C1>> + 'a,
        C2: re_types_core::Component + Clone + Into<::std::borrow::Cow<'a, C2>> + 'a,
    >(
        &self,
    ) -> crate::Result<DataCellRow> {
        let cell0 = DataCell::from_native(self.iter_instance_keys());
        let cell1 = DataCell::from_native_sparse(self.iter_optional_component::<C1>()?);
        let cell2 = DataCell::from_native_sparse(self.iter_optional_component::<C2>()?);
        Ok(DataCellRow(smallvec::smallvec![cell0, cell1, cell2]))
    }
}

#[test]
fn lookup_value() {
    use re_types::components::{Color, InstanceKey, Position2D};

    let instance_keys = InstanceKey::from_iter(0..5);

    let points = [
        Position2D::new(1.0, 2.0), //
        Position2D::new(3.0, 4.0),
        Position2D::new(5.0, 6.0),
        Position2D::new(7.0, 8.0),
        Position2D::new(9.0, 10.0),
    ];

    let component = ComponentWithInstances::from_native(instance_keys, points).unwrap();

    let missing_value = component.lookup_arrow(&InstanceKey(5));
    assert_eq!(missing_value, None);

    let value = component.lookup_arrow(&InstanceKey(2)).unwrap();

    let expected_point = [points[2]];
    let expected_arrow = Position2D::to_arrow(expected_point).unwrap();

    assert_eq!(expected_arrow, value);

    let instance_keys = [
        InstanceKey(17),
        InstanceKey(47),
        InstanceKey(48),
        InstanceKey(99),
        InstanceKey(472),
    ];

    let component = ComponentWithInstances::from_native(instance_keys, points).unwrap();

    let missing_value = component.lookup_arrow(&InstanceKey(46));
    assert_eq!(missing_value, None);

    let value = component.lookup_arrow(&InstanceKey(99)).unwrap();

    let expected_point = [points[3]];
    let expected_arrow = Position2D::to_arrow(expected_point).unwrap();

    assert_eq!(expected_arrow, value);

    // Lookups with serialization

    let value = component.lookup::<Position2D>(&InstanceKey(99)).unwrap();
    assert_eq!(expected_point[0], value);

    let missing_value = component.lookup::<Position2D>(&InstanceKey(46));
    assert!(matches!(
        missing_value.err().unwrap(),
        QueryError::ComponentNotFound(_)
    ));

    let missing_value = component.lookup::<Color>(&InstanceKey(99));
    assert!(matches!(
        missing_value.err().unwrap(),
        QueryError::TypeMismatch { .. }
    ));
}

#[test]
fn lookup_splat() {
    use re_types::components::{InstanceKey, Position2D};
    let instances = [
        InstanceKey::SPLAT, //
    ];
    let points = [
        Position2D::new(1.0, 2.0), //
    ];

    let component = ComponentWithInstances::from_native(instances, points).unwrap();

    // Any instance we look up will return the slatted value
    let value = component.lookup::<Position2D>(&InstanceKey(1)).unwrap();
    assert_eq!(points[0], value);

    let value = component.lookup::<Position2D>(&InstanceKey(99)).unwrap();
    assert_eq!(points[0], value);
}
