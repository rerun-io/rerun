use nohash_hasher::IntMap;
use re_log_types::{DataCell, RowId, TimeInt};
use re_types_core::ComponentName;
use re_types_core::{Component, DeserializationError, DeserializationResult};

use crate::{Promise, PromiseResolver, PromiseResult};

// ---

/// Raw results for a latest-at query.
///
/// The data is neither deserialized, nor resolved/converted.
/// It it the raw [`DataCell`]s, straight from our datastore.
///
/// Use [`LatestAtResults::get`], [`LatestAtResults::get_required`] and [`LatestAtResults::get_optional`]
/// in order to access the raw results for each individual component.
#[derive(Debug, Clone)]
pub struct LatestAtResults {
    /// The compound index of this query result.
    ///
    /// A latest-at query is a compound operation that gathers data from many different rows.
    /// The index of that compound result corresponds to the index of most recent row in all the
    /// sub-results.
    pub compound_index: (TimeInt, RowId),

    /// Raw results for each individual component.
    pub components: IntMap<ComponentName, LatestAtComponentResults>,
}

impl Default for LatestAtResults {
    #[inline]
    fn default() -> Self {
        Self {
            compound_index: (TimeInt::STATIC, RowId::ZERO),
            components: Default::default(),
        }
    }
}

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
            static DEFAULT: LatestAtComponentResults = LatestAtComponentResults::empty();
            &DEFAULT
        }
    }
}

impl LatestAtResults {
    #[doc(hidden)]
    #[inline]
    pub fn add(&mut self, component_name: ComponentName, index: (TimeInt, RowId), cell: DataCell) {
        let (data_time, row_id) = index;
        let (compound_data_time, _) = self.compound_index;

        // NOTE: Since this is a compound API that actually emits multiple queries, the data time of the
        // final result is the most recent data time among all of its components.
        if data_time > compound_data_time {
            self.compound_index = (data_time, row_id);
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

/// Uncached results for a particular component when using a latest-at query.
#[derive(Debug, Clone)]
pub struct LatestAtComponentResults {
    index: (TimeInt, RowId),

    // Option so we can have a constant default value for `Self` for the optional+empty case.
    cell: Option<Promise>,
}

impl Default for LatestAtComponentResults {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

impl LatestAtComponentResults {
    #[inline]
    const fn empty() -> Self {
        Self {
            index: (TimeInt::STATIC, RowId::ZERO),
            cell: None,
        }
    }
}

impl LatestAtComponentResults {
    #[inline]
    pub fn index(&self) -> &(TimeInt, RowId) {
        &self.index
    }

    /// Returns the component data as a dense vector.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    ///
    /// Use [`PromiseResult::flatten`] to merge the results of resolving the promise and of
    /// deserializing the data into a single one, if you don't need the extra flexibility.
    #[inline]
    pub fn to_dense<C: Component>(
        &self,
        resolver: &mut PromiseResolver,
    ) -> PromiseResult<DeserializationResult<Vec<C>>> {
        if let Some(cell) = self.cell.as_ref() {
            resolver.resolve(cell).map(|cell| {
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
    ///
    /// Use [`PromiseResult::flatten`] to merge the results of resolving the promise and of
    /// deserializing the data into a single one, if you don't need the extra flexibility.
    #[inline]
    pub fn iter_dense<C: Component>(
        &self,
        resolver: &mut PromiseResolver,
    ) -> PromiseResult<DeserializationResult<impl ExactSizeIterator<Item = C>>> {
        self.to_dense(resolver)
            .map(|data| data.map(|data| data.into_iter()))
    }

    /// Returns the component data as a sparse vector.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    ///
    /// Use [`PromiseResult::flatten`] to merge the results of resolving the promise and of
    /// deserializing the data into a single one, if you don't need the extra flexibility.
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
            resolver.resolve(cell).map(|cell| {
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
    ///
    /// Use [`PromiseResult::flatten`] to merge the results of resolving the promise and of
    /// deserializing the data into a single one, if you don't need the extra flexibility.
    #[inline]
    pub fn iter_sparse<C: Component>(
        &self,
        resolver: &mut PromiseResolver,
    ) -> PromiseResult<DeserializationResult<impl ExactSizeIterator<Item = Option<C>>>> {
        self.to_sparse(resolver)
            .map(|data| data.map(|data| data.into_iter()))
    }
}
