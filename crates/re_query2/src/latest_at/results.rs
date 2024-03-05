use nohash_hasher::IntMap;
use re_log_types::{DataCell, RowId, TimeInt};
use re_types_core::ComponentName;
use re_types_core::{Component, DeserializationError, DeserializationResult};

use crate::{Promise, PromiseResolver, PromiseResult};

// ---

/// Results for a latest-at query.
#[derive(Debug, Clone)]
pub struct LatestAtResults {
    pub max_index: (Option<TimeInt>, RowId),
    pub components: IntMap<ComponentName, LatestAtComponentResults>,
}

// impl crate::Promise for LatestAtResults {
//     type Output = Self;
//
//     fn resolve(&mut self) -> crate::PromiseResult<Self::Output> {
//         // TODO: you'd be resolving stuff here, somehow.
//         todo!()
//     }
// }

impl Default for LatestAtResults {
    #[inline]
    fn default() -> Self {
        Self {
            max_index: (None, RowId::ZERO),
            components: Default::default(),
        }
    }
}

// TODO: how do promises fit into that?

impl LatestAtResults {
    #[inline]
    pub fn contains(&self, component_name: impl Into<ComponentName>) -> bool {
        self.components.contains_key(&component_name.into())
    }

    /// Returns the [`LatestAtComponentResults`] for the specified [`Component`].
    #[inline]
    pub fn get<C: Component>(&self) -> Option<&LatestAtComponentResults> {
        self.components.get(&C::name())
    }

    /// Returns the [`LatestAtComponentResults`] for the specified [`Component`].
    ///
    /// Returns an error if the component is not present.
    #[inline]
    pub fn get_required<C: Component>(&self) -> crate::Result<&LatestAtComponentResults> {
        if let Some(component) = self.components.get(&C::name()) {
            Ok(component)
        } else {
            Err(DeserializationError::MissingComponent {
                component: C::name(),
                backtrace: ::backtrace::Backtrace::new_unresolved(),
            }
            .into())
        }
    }

    /// Returns the [`LatestAtComponentResults`] for the specified [`Component`].
    ///
    /// Returns empty results if the component is not present.
    #[inline]
    pub fn get_optional<C: Component>(&self) -> &LatestAtComponentResults {
        if let Some(component) = self.components.get(&C::name()) {
            component
        } else {
            static DEFAULT: LatestAtComponentResults = LatestAtComponentResults::uninit();
            &DEFAULT
        }
    }
}

impl LatestAtResults {
    #[doc(hidden)]
    #[inline]
    pub fn add(
        &mut self,
        component_name: ComponentName,
        index: (Option<TimeInt>, RowId),
        cell: DataCell,
    ) {
        let (data_time, row_id) = index;
        let (max_data_time, max_row_id) = &mut self.max_index;

        // NOTE: Since this is a compound API that actually emits multiple queries, the data time of the
        // final result is the most recent data time among all of its components.
        if data_time > *max_data_time {
            *max_data_time = (*max_data_time).max(data_time);
            // TODO: max_row_id is a shit name
            *max_row_id = row_id;
        }

        self.components.insert(
            component_name,
            LatestAtComponentResults {
                index,
                cell: Some(Promise::new(cell)),
            },
        );
    }
}

// ---

// TODO: should this be a trait, for promises?

// TODO: this is where we actually downcast data -- at some point in here we have to decide where
// we resolve promises?!

/// Uncached results for a particular component when using a latest-at query.
#[derive(Debug, Clone)]
pub struct LatestAtComponentResults {
    index: (Option<TimeInt>, RowId),

    // Option so we can have a constant default value for `Self` for the optional+empty case.
    cell: Option<Promise>,
}

impl LatestAtComponentResults {
    #[inline]
    const fn uninit() -> Self {
        Self {
            index: (None, RowId::ZERO),
            cell: None,
        }
    }
}

impl LatestAtComponentResults {
    #[inline]
    pub fn index(&self) -> &(Option<TimeInt>, RowId) {
        &self.index
    }

    // TODO: can we remove a layer somehow..? keep them, in that same "escape hatch spirit"

    /// Returns the component data as a dense vector.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    #[inline]
    pub fn to_dense<C: Component>(
        &self,
        resolver: &mut PromiseResolver,
    ) -> PromiseResult<DeserializationResult<Vec<C>>> {
        if let Some(cell) = self.cell.as_ref() {
            resolver.resolve(cell).map_ok(|cell| {
                cell.try_to_native()
                    .map_err(|err| DeserializationError::DataCellError(err.to_string()))
            })
        } else {
            // Manufactured empty result.
            PromiseResult::Ready(Ok(vec![]))
        }
    }

    /// Iterates over the component data, assuming it is dense.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    #[inline]
    pub fn iter_dense<C: Component>(
        &self,
        resolver: &mut PromiseResolver,
    ) -> PromiseResult<DeserializationResult<impl ExactSizeIterator<Item = C>>> {
        self.to_dense(resolver)
            .map_ok(|data| data.map(|data| data.into_iter()))
    }

    /// Returns the component data as a sparse vector.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    #[inline]
    pub fn to_sparse<C: Component>(
        &self,
        resolver: &mut PromiseResolver,
    ) -> PromiseResult<DeserializationResult<Vec<Option<C>>>> {
        // Manufactured empty result.
        if self.cell.is_none() {
            return PromiseResult::Ready(Ok(vec![]));
        }

        if let Some(cell) = self.cell.as_ref() {
            resolver.resolve(cell).map_ok(|cell| {
                cell.try_to_native_opt()
                    .map_err(|err| DeserializationError::DataCellError(err.to_string()))
            })
        } else {
            // Manufactured empty result.
            PromiseResult::Ready(Ok(vec![]))
        }
    }

    /// Iterates over the component data, assuming it is sparse.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    #[inline]
    pub fn iter_sparse<C: Component>(
        &self,
        resolver: &mut PromiseResolver,
    ) -> PromiseResult<DeserializationResult<impl ExactSizeIterator<Item = Option<C>>>> {
        self.to_sparse(resolver)
            .map_ok(|data| data.map(|data| data.into_iter()))
    }
}
