use std::collections::VecDeque;
use std::sync::{Arc, OnceLock};

use nohash_hasher::IntMap;
use parking_lot::{RwLock, RwLockReadGuard};
use re_log_types::{DataCell, RowId, TimeInt};
use re_query2::QueryError;
use re_types_core::ComponentName;
use re_types_core::{Component, DeserializationError};

use crate::{ErasedFlatVecDeque, FlatVecDeque};

// ---

/// Results for a cached latest-at query.
#[derive(Debug)]
pub struct CachedRangeResults {
    pub max_index: (Option<TimeInt>, RowId),
    pub components: IntMap<ComponentName, Arc<RwLock<CachedRangeComponentResults>>>,
}

impl Default for CachedRangeResults {
    #[inline]
    fn default() -> Self {
        Self {
            max_index: (None, RowId::ZERO),
            components: Default::default(),
        }
    }
}

// TODO: fuck it, we'll return guards if we must.

impl CachedRangeResults {
    #[inline]
    pub fn contains(&self, component_name: impl Into<ComponentName>) -> bool {
        self.components.contains_key(&component_name.into())
    }

    /// Returns the [`RangeComponentResults`] for the specified [`Component`].
    #[inline]
    pub fn get<C: Component>(&self) -> Option<RwLockReadGuard<'_, CachedRangeComponentResults>> {
        self.components
            .get(&C::name())
            .map(|lock| lock.read_recursive())
    }

    /// Returns the [`CachedRangeComponentResults`] for the specified [`Component`].
    ///
    /// Returns an error if the component is not present.
    #[inline]
    pub fn get_required<C: Component>(
        &self,
    ) -> crate::Result<RwLockReadGuard<'_, CachedRangeComponentResults>> {
        if let Some(component) = self.components.get(&C::name()) {
            Ok(component.read_recursive())
        } else {
            Err(DeserializationError::MissingComponent {
                component: C::name(),
                backtrace: ::backtrace::Backtrace::new_unresolved(),
            }
            .into())
        }
    }

    /// Returns the [`CachedRangeComponentResults`] for the specified [`Component`].
    ///
    /// Returns empty results if the component is not present.
    #[inline]
    pub fn get_optional<C: Component>(&self) -> RwLockReadGuard<'_, CachedRangeComponentResults> {
        if let Some(component) = self.components.get(&C::name()) {
            component.read_recursive()
        } else {
            static DEFAULT: RwLock<CachedRangeComponentResults> =
                RwLock::new(CachedRangeComponentResults::new());
            DEFAULT.read()
        }
    }
}

impl CachedRangeResults {
    #[doc(hidden)]
    #[inline]
    pub fn add(
        &mut self,
        component_name: ComponentName,
        data: impl Iterator<Item = ((Option<TimeInt>, RowId), DataCell)>,
    ) {
        let mut indices = VecDeque::new();
        let mut cells = VecDeque::new();

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

            indices.push_back(index);
            cells.push_back(cell);
        }

        // TODO: .unzip() would require Default
        // let (indices, cells) = data.unzip();

        let results = CachedRangeComponentResults {
            indices,
            cells,
            cached: Default::default(),
        };
        results.sanity_check();

        self.components
            .insert(component_name, Arc::new(RwLock::new(results)));
    }
}

// ---

// TODO: how tf are we supposed to handle ranged promises?
// that means the primary cache would have to cache the results coming from the promise-cache, so
// it cannot be considered a secondary anymore or nothing makes sense

// TODO: one limitation would be to say that a single component of a single entity must be either
// all promises or all values...
// or we can make `cached` a `Vec<ErasedFlatVecDeque>` and make a cut each time the actual type
// changes

/// Lazily cached results for a particular component when using a cached latest-at query.
#[derive(Default)]
pub struct CachedRangeComponentResults {
    pub indices: VecDeque<(Option<TimeInt>, RowId)>,
    pub cells: VecDeque<DataCell>,
    pub cached: OnceLock<Box<dyn ErasedFlatVecDeque + Send + Sync>>,
}

impl CachedRangeComponentResults {
    #[inline]
    pub const fn new() -> Self {
        Self {
            indices: VecDeque::new(),
            cells: VecDeque::new(),
            cached: OnceLock::new(),
        }
    }

    /// No-op in release.
    #[inline]
    pub fn sanity_check(&self) {
        let Self {
            indices,
            cells,
            cached: _,
        } = self;

        if cfg!(debug_assertions) {
            assert_eq!(indices.len(), cells.len());
        }
    }
}

impl std::fmt::Debug for CachedRangeComponentResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            indices,
            cells,
            cached: _, // we can't, we don't know the type
        } = self;

        f.debug_struct("CachedComponentData")
            .field("indices", &indices)
            .field("cells", &cells)
            .finish()
    }
}

// TODO: this is where we do the actual upsert!

// TODO: iters only because one cannot simply slice a deque

impl CachedRangeComponentResults {
    #[inline]
    pub fn iter_indices(&self) -> impl ExactSizeIterator<Item = (Option<TimeInt>, RowId)> + '_ {
        self.indices.iter().copied()
    }

    /// Iterates over the component data, assuming it is dense.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    #[inline]
    pub fn iter_dense<C: 'static + Component + Send + Sync>(
        &self,
    ) -> crate::Result<impl Iterator<Item = &[C]>> {
        self.downcast_dense::<C>()
    }

    /// Iterates over the component data, assuming it is sparse.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    #[inline]
    pub fn iter_sparse<C: 'static + Component + Send + Sync>(
        &self,
    ) -> crate::Result<impl Iterator<Item = &[Option<C>]>> {
        self.downcast_sparse::<C>()
    }
}

impl CachedRangeComponentResults {
    fn downcast_dense<C: 'static + Component + Send + Sync>(
        &self,
    ) -> crate::Result<impl Iterator<Item = &[C]>> {
        // TODO: this is non-blocking -- best-effort to avoid deserializing for no reason
        if let Some(cached) = self.cached.get() {
            let cached = cached
                .as_any()
                .downcast_ref::<FlatVecDeque<C>>()
                .ok_or_else(|| QueryError::TypeMismatch {
                    actual: "<unknown>".into(),
                    requested: C::name(),
                })?;

            // unwrap checked just above ^^^
            return Ok(cached.iter());
        }

        // self.cells.iter().map(|cell| {

        // let data = self
        //     .cell
        //     .try_to_native::<C>()
        //     .map_err(|err| DeserializationError::DataCellError(err.to_string()))?;
        //
        // #[allow(clippy::borrowed_box)]
        // let cached: &Box<dyn ErasedFlatVecDeque + Send + Sync> = self
        //     .cached
        //     .get_or_init(move || Box::new(FlatVecDeque::from(data)));

        // TODO: obviously not gonna cut it
        #[allow(clippy::borrowed_box)]
        let cached: &Box<dyn ErasedFlatVecDeque + Send + Sync> = self
            .cached
            .get_or_init(move || Box::new(FlatVecDeque::<C>::new()));

        let cached = cached
            .as_any()
            .downcast_ref::<FlatVecDeque<C>>()
            .ok_or_else(|| QueryError::TypeMismatch {
                actual: "<unknown>".into(),
                requested: C::name(),
            })?;

        Ok(cached.iter())
    }

    fn downcast_sparse<C: 'static + Component + Send + Sync>(
        &self,
    ) -> crate::Result<impl Iterator<Item = &[Option<C>]>> {
        // TODO: this is non-blocking -- best-effort to avoid deserializing for no reason
        if let Some(cached) = self.cached.get() {
            let cached = cached
                .as_any()
                .downcast_ref::<FlatVecDeque<Option<C>>>()
                .ok_or_else(|| QueryError::TypeMismatch {
                    actual: "<unknown>".into(),
                    requested: C::name(),
                })?;
            return Ok(cached.iter());
        }

        // let data = self
        //     .cell
        //     .try_to_native_opt::<C>()
        //     .map_err(|err| DeserializationError::DataCellError(err.to_string()))?;
        //
        // #[allow(clippy::borrowed_box)]
        // let cached: &Box<dyn ErasedFlatVecDeque + Send + Sync> = self
        //     .cached
        //     .get_or_init(move || Box::new(FlatVecDeque::from(data)));

        #[allow(clippy::borrowed_box)]
        let cached: &Box<dyn ErasedFlatVecDeque + Send + Sync> = self
            .cached
            .get_or_init(move || Box::new(FlatVecDeque::<Option<C>>::new()));

        let cached = cached
            .as_any()
            .downcast_ref::<FlatVecDeque<Option<C>>>()
            .ok_or_else(|| QueryError::TypeMismatch {
                actual: "<unknown>".into(),
                requested: C::name(),
            })?;

        Ok(cached.iter())
    }
}
