use nohash_hasher::IntMap;
use re_log_types::{DataCell, RowId, TimeInt};
use re_types_core::ComponentName;
use re_types_core::{Component, DeserializationError, DeserializationResult};

use crate::{Promise, PromiseResolver, PromiseResult};

// ---

/// Results for a latest-at query.
#[derive(Debug, Clone)]
pub struct RangeResults {
    pub max_index: (Option<TimeInt>, RowId),
    pub components: IntMap<ComponentName, RangeComponentResults>,
}

impl Default for RangeResults {
    #[inline]
    fn default() -> Self {
        Self {
            max_index: (None, RowId::ZERO),
            components: Default::default(),
        }
    }
}

impl RangeResults {
    #[inline]
    pub fn contains(&self, component_name: impl Into<ComponentName>) -> bool {
        self.components.contains_key(&component_name.into())
    }

    /// Returns the [`RangeComponentResults`] for the specified [`Component`].
    #[inline]
    pub fn get<C: Component>(&self) -> Option<&RangeComponentResults> {
        self.components.get(&C::name())
    }

    /// Returns the [`RangeComponentResults`] for the specified [`Component`].
    ///
    /// Returns an error if the component is not present.
    #[inline]
    pub fn get_required<C: Component>(&self) -> crate::Result<&RangeComponentResults> {
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

    /// Returns the [`RangeComponentResults`] for the specified [`Component`].
    ///
    /// Returns empty results if the component is not present.
    #[inline]
    pub fn get_optional<C: Component>(&self) -> &RangeComponentResults {
        if let Some(component) = self.components.get(&C::name()) {
            component
        } else {
            static DEFAULT: RangeComponentResults = RangeComponentResults::new();
            &DEFAULT
        }
    }
}

impl RangeResults {
    #[doc(hidden)]
    #[inline]
    pub fn add(
        &mut self,
        component_name: ComponentName,
        data: impl Iterator<Item = ((Option<TimeInt>, RowId), DataCell)>,
    ) {
        let mut indices = Vec::new();
        let mut cells = Vec::new();

        for (index, cell) in data {
            // TODO: any point doing this for range?

            let (data_time, row_id) = index;
            let (max_data_time, max_row_id) = &mut self.max_index;

            // NOTE: Since this is a compound API that actually emits multiple queries, the data time of the
            // final result is the most recent data time among all of its components.
            if data_time > *max_data_time {
                *max_data_time = (*max_data_time).max(data_time);
                // TODO: max_row_id is a shit name
                *max_row_id = row_id;
            }

            indices.push(index);
            cells.push(Promise::new(cell));
        }

        // TODO: .unzip() would require Default
        // let (indices, cells) = data.unzip();

        let results = RangeComponentResults { indices, cells };
        results.sanity_check();

        self.components.insert(component_name, results);
    }
}

// ---

// TODO: sure, it's wasteful, but if perfomance mattered, you wouldn't be using the
// uncached APIs... right?

// TODO: I'd love to make this generic but by definition we need this untyped

/// Uncached results for a particular component when using a latest-at query.
#[derive(Debug, Clone)]
pub struct RangeComponentResults {
    pub indices: Vec<(Option<TimeInt>, RowId)>,
    pub cells: Vec<Promise>,
}

impl Default for RangeComponentResults {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl RangeComponentResults {
    #[inline]
    pub const fn new() -> Self {
        Self {
            indices: Vec::new(),
            cells: Vec::new(),
        }
    }

    /// No-op in release.
    #[inline]
    pub fn sanity_check(&self) {
        let Self { indices, cells } = self;
        if cfg!(debug_assertions) {
            assert_eq!(indices.len(), cells.len());
        }
    }
}

impl RangeComponentResults {
    #[inline]
    pub fn indices(&self) -> &[(Option<TimeInt>, RowId)] {
        &self.indices
    }

    #[inline]
    pub fn iter_indices(&self) -> impl ExactSizeIterator<Item = (Option<TimeInt>, RowId)> + '_ {
        self.indices.iter().copied()
    }

    /// Returns the component data as a dense vector.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    #[inline]
    pub fn to_dense<C: Component>(
        &self,
        resolver: &mut PromiseResolver,
    ) -> Vec<PromiseResult<DeserializationResult<Vec<C>>>> {
        self.cells
            .iter()
            .map(|cell| {
                resolver.resolve(cell).map_ok(|cell| {
                    cell.try_to_native()
                        .map_err(|err| DeserializationError::DataCellError(err.to_string()))
                })
            })
            .collect()
    }

    /// Iterates over the component data, assuming it is dense.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    #[inline]
    pub fn iter_dense<C: Component>(
        &self,
        resolver: &mut PromiseResolver,
    ) -> impl ExactSizeIterator<Item = PromiseResult<DeserializationResult<Vec<C>>>> {
        self.to_dense(resolver).into_iter()
    }

    /// Returns the component data as a sparse vector.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    #[inline]
    pub fn to_sparse<C: Component>(
        &self,
        resolver: &mut PromiseResolver,
    ) -> Vec<PromiseResult<DeserializationResult<Vec<Option<C>>>>> {
        self.cells
            .iter()
            .map(|cell| {
                resolver.resolve(cell).map_ok(|cell| {
                    cell.try_to_native_opt()
                        .map_err(|err| DeserializationError::DataCellError(err.to_string()))
                })
            })
            .collect()
    }

    /// Iterates over the component data, assuming it is sparse.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    #[inline]
    pub fn iter_sparse<C: Component>(
        &self,
        resolver: &mut PromiseResolver,
    ) -> impl ExactSizeIterator<Item = PromiseResult<DeserializationResult<Vec<Option<C>>>>> {
        self.to_sparse(resolver).into_iter()
    }
}
