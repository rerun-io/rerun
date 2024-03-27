use std::{
    collections::VecDeque,
    ops::Range,
    sync::{
        atomic::{AtomicU64, Ordering::Relaxed},
        Arc, OnceLock,
    },
};

use itertools::Either;
use nohash_hasher::IntMap;

use parking_lot::{MappedRwLockReadGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};
use re_log_types::{DataCell, RowId, TimeInt, TimeRange};
use re_types_core::{Component, ComponentName, DeserializationError, SizeBytes};

use crate::{
    cache, ErasedFlatVecDeque, FlatVecDeque, Promise, PromiseResolver, PromiseResult, QueryError,
};

// ---

/// Cached results for a range query.
///
/// The data is both deserialized and resolved/converted.
///
/// Use [`CachedRangeResults::get`], [`CachedRangeResults::get_required`] and
/// [`CachedRangeResults::get_optional`] in order to access the results for each individual component.
#[derive(Debug)]
pub struct CachedRangeResults {
    /// Raw results for each individual component.
    pub components: IntMap<ComponentName, CachedRangeComponentResults>,
}

impl Default for CachedRangeResults {
    #[inline]
    fn default() -> Self {
        Self {
            components: Default::default(),
        }
    }
}

// TODO: still gotta figure out whether we _need_ the mutex recursiveness here...

impl CachedRangeResults {
    #[inline]
    pub fn contains(&self, component_name: impl Into<ComponentName>) -> bool {
        self.components.contains_key(&component_name.into())
    }

    /// Returns the [`CachedRangeComponentResults`] for the specified [`Component`].
    #[inline]
    pub fn get(
        &self,
        component_name: impl Into<ComponentName>,
    ) -> Option<&CachedRangeComponentResults> {
        self.components.get(&component_name.into())
    }

    /// Returns the [`CachedRangeComponentResults`] for the specified [`Component`].
    ///
    /// Returns an error if the component is not present.
    #[inline]
    pub fn get_required(
        &self,
        component_name: impl Into<ComponentName>,
    ) -> crate::Result<&CachedRangeComponentResults> {
        let component_name = component_name.into();
        if let Some(component) = self.components.get(&component_name) {
            Ok(component)
        } else {
            Err(DeserializationError::MissingComponent {
                component: component_name,
                backtrace: ::backtrace::Backtrace::new_unresolved(),
            }
            .into())
        }
    }

    /// Returns the [`CachedRangeComponentResults`] for the specified [`Component`].
    ///
    /// Returns empty results if the component is not present.
    #[inline]
    pub fn get_optional(
        &self,
        component_name: impl Into<ComponentName>,
    ) -> &CachedRangeComponentResults {
        let component_name = component_name.into();
        if let Some(component) = self.components.get(&component_name) {
            component
        } else {
            static EMPTY: OnceLock<CachedRangeComponentResults> = OnceLock::new();
            EMPTY.get_or_init(|| {
                Arc::new(RwLock::new(CachedRangeComponentResultsInner::empty())).into()
            })
        }
    }
}

impl CachedRangeResults {
    #[doc(hidden)]
    #[inline]
    pub fn add(&mut self, component_name: ComponentName, cached: CachedRangeComponentResults) {
        self.components.insert(component_name, cached);
    }
}

// ---

// TODO: this one should probably be renamed, and the other should be made private
#[derive(Debug, Clone)]
pub struct CachedRangeComponentResults(Arc<RwLock<CachedRangeComponentResultsInner>>);

impl Default for CachedRangeComponentResults {
    #[inline]
    fn default() -> Self {
        Self(Arc::new(RwLock::new(
            CachedRangeComponentResultsInner::empty(),
        )))
    }
}

impl std::ops::Deref for CachedRangeComponentResults {
    type Target = RwLock<CachedRangeComponentResultsInner>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Arc<RwLock<CachedRangeComponentResultsInner>>> for CachedRangeComponentResults {
    #[inline]
    fn from(results: Arc<RwLock<CachedRangeComponentResultsInner>>) -> Self {
        Self(results)
    }
}

/// Lazily cached results for a particular component when using a cached range query.
pub struct CachedRangeComponentResultsInner {
    pub(crate) indices: VecDeque<(TimeInt, RowId)>,

    // TODO: explain terminology
    pub(crate) promises_front: Vec<((TimeInt, RowId), Promise)>,

    // TODO: explain terminology
    pub(crate) promises_back: Vec<((TimeInt, RowId), Promise)>,

    // TODO: pending insertions somehow?

    // TODO: that option is a mess -- explain why we have it
    //
    /// The resolved, converted, deserialized dense data.
    pub(crate) cached_dense: Option<Box<dyn ErasedFlatVecDeque + Send + Sync>>,

    /// The resolved, converted, deserialized sparse data.
    pub(crate) cached_sparse: Option<Box<dyn ErasedFlatVecDeque + Send + Sync>>,

    pub(crate) cached_heap_size_bytes: AtomicU64,
}

impl CachedRangeComponentResultsInner {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            indices: VecDeque::new(),
            promises_front: Vec::new(),
            promises_back: Vec::new(),
            cached_dense: None,
            cached_sparse: None,
            cached_heap_size_bytes: AtomicU64::new(0),
        }
    }

    /// No-op in release.
    #[inline]
    pub fn sanity_check(&self) {
        let Self {
            indices,
            promises_front: _,
            promises_back: _,
            cached_dense,
            cached_sparse,
            cached_heap_size_bytes: _,
        } = self;

        if let Some(dense) = cached_dense.as_ref() {
            assert_eq!(indices.len(), dense.dyn_num_entries());
        }

        if let Some(sparse) = cached_sparse.as_ref() {
            assert_eq!(indices.len(), sparse.dyn_num_entries());
        }
    }

    #[inline]
    pub fn time_range(&self) -> Option<TimeRange> {
        let first_time = self.indices.front().map(|(t, _)| *t)?;
        let last_time = self.indices.back().map(|(t, _)| *t)?;
        Some(TimeRange::new(first_time, last_time))
    }

    #[inline]
    pub fn contains_data_time(&self, data_time: TimeInt) -> bool {
        let first_time = self.indices.front().map_or(&TimeInt::MAX, |(t, _)| t);
        let last_time = self.indices.back().map_or(&TimeInt::MIN, |(t, _)| t);
        *first_time <= data_time && data_time <= *last_time
    }

    // TODO: ??
    // /// Returns the [`ComponentName`] of the resolved data, if available.
    // #[inline]
    // pub fn component_name(&self, resolver: &PromiseResolver) -> Option<ComponentName> {
    //     match self.resolved(resolver) {
    //         PromiseResult::Ready(cell) => Some(cell.component_name()),
    //         _ => None,
    //     }
    // }
}

impl SizeBytes for CachedRangeComponentResultsInner {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.cached_heap_size_bytes.load(Relaxed)
    }
}

impl std::fmt::Debug for CachedRangeComponentResultsInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            indices,
            promises_front,
            promises_back,
            cached_dense: _,  // we can't, we don't know the type
            cached_sparse: _, // we can't, we don't know the type
            cached_heap_size_bytes,
        } = self;

        f.debug_tuple("CachedRangeComponentResults")
            .field(&indices)
            .finish()

        // f.write_fmt(format_args!(
        //     "[{:?}#{}] {}",
        //     index.0,
        //     index.1,
        //     re_format::format_bytes(cached_heap_size_bytes.load(Relaxed) as _)
        // ))
    }
}

// TODO: this type is where we could indicate whether we're missing data -- either on the front or
// in the back.
pub struct CachedRangeData<'a, T> {
    indices: MappedRwLockReadGuard<'a, VecDeque<(TimeInt, RowId)>>,
    data: MappedRwLockReadGuard<'a, FlatVecDeque<T>>,
}

impl<'a, T> CachedRangeData<'a, T> {
    #[inline]
    pub fn indices(&self, entry_range: Range<usize>) -> impl Iterator<Item = &(TimeInt, RowId)> {
        self.indices.range(entry_range)
    }

    #[inline]
    pub fn data(&self, entry_range: Range<usize>) -> impl Iterator<Item = &[T]> {
        self.data.range(entry_range)
    }

    #[inline]
    pub fn indexed(
        &self,
        time_range: TimeRange,
    ) -> impl Iterator<Item = (&(TimeInt, RowId), &[T])> {
        let entry_range = self.entry_range(time_range);
        itertools::izip!(self.indices(entry_range.clone()), self.data(entry_range))
    }

    /// Returns the index range that corresponds to the specified `time_range`.
    ///
    /// Use the returned range with one of the range iteration methods:
    /// - [`Self::range_data_times`]
    /// - [`Self::range_pov_instance_keys`]
    /// - [`Self::range_component`]
    /// - [`Self::range_component_opt`]
    ///
    /// Make sure that the bucket hasn't been modified in-between!
    ///
    /// This is `O(2*log(n))`, so make sure to clone the returned range rather than calling this
    /// multiple times.
    #[inline]
    pub fn entry_range(&self, time_range: TimeRange) -> Range<usize> {
        let start_index = self
            .indices
            .partition_point(|(data_time, _)| data_time < &time_range.min());
        let end_index = self
            .indices
            .partition_point(|(data_time, _)| data_time <= &time_range.max());
        start_index..end_index
    }
}

// TODO: i guess somehow we need to explain that switching between dense and sparse is UB? we dont
// use it anyway though we who cares really...

impl CachedRangeComponentResults {
    // #[inline]
    // pub fn indices(&self) -> MappedRwLockReadGuard<'_, VecDeque<(TimeInt, RowId)>> {
    //     RwLockReadGuard::map(self.0.read_recursive(), |inner| &inner.indices)
    // }

    // TODO(Amanieu/parking_lot#289): we cannot yet express the following because of a limitation
    // in `parking_lot`.
    // See <https://github.com/Amanieu/parking_lot/issues/289#issuecomment-1827545967>.
    //
    // #[inline]
    // pub fn iter_indices(
    //     &self,
    // ) -> MappedRwLockReadGuard<'_, impl Iterator<Item = (TimeInt, RowId)> + '_> {
    //     MappedRwLockReadGuard::map(self.indices(), |indices| indices.iter().copied())
    // }

    // TODO: no idea what that would look like in this case.
    // /// Returns the raw resolved data, if it's ready.
    // #[inline]
    // pub fn resolved(&self, resolver: &PromiseResolver) -> PromiseResult<DataCell> {
    //     if let Some(cell) = self.promise.as_ref() {
    //         resolver.resolve(cell)
    //     } else {
    //         PromiseResult::Pending
    //     }
    // }

    // TODO: we still need a way to slice it somehow though, otherwise we're returning the whole
    // range all the time

    /// Returns the component data as a dense vector.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    ///
    /// Use [`PromiseResult::flatten`] to merge the results of resolving the promise and of
    /// deserializing the data into a single one, if you don't need the extra flexibility.
    //
    // TODO: rm debug clause
    #[inline]
    pub fn to_dense<C: Component + std::fmt::Debug>(
        &self,
        resolver: &PromiseResolver,
    ) -> CachedRangeData<'_, C> {
        // TODO: probably best to make sure we always hold both at once here.

        // --- Step 1: upsert pending data (write lock) ---

        // TODO: should probably be a try_write for the same reasons as before?
        let mut results = self.0.write();

        if results.cached_dense.is_none() {
            results.cached_dense = Some(Box::new(FlatVecDeque::<C>::new()));
        }

        if !results.promises_front.is_empty() {
            let mut resolved_indices = Vec::with_capacity(results.promises_front.len());
            let mut resolved_data = Vec::with_capacity(results.promises_front.len());

            // TODO: this is broken -- drain will drop everything if we stop halfway

            // TODO: we walk in reverse so that we can break on pending at the edge
            for (index, promise) in results.promises_front.drain(..).rev() {
                let data = match resolver.resolve(&promise) {
                    PromiseResult::Pending => {
                        unreachable!();
                        // TODO: stop there, we'll try again from here next time around
                        break;
                    }
                    // TODO: for now i think this is fine.. but in the future maybe we want to
                    // "ignore" this one and go on?
                    PromiseResult::Error(err) => {
                        // TODO: warning data dropped
                        re_log::error!(%err);
                        continue;
                    }
                    PromiseResult::Ready(cell) => {
                        match cell
                            .try_to_native::<C>()
                            .map_err(|err| DeserializationError::DataCellError(err.to_string()))
                        {
                            Ok(data) => data,
                            Err(err) => {
                                // TODO: warning data dropped
                                re_log::error!(%err);
                                continue;
                            }
                        }
                    }
                };

                resolved_indices.push(index);
                resolved_data.push(data);
            }

            // TODO: uh-oh!! we can't be updating indices here and have indices() be its own method!!

            // TODO: because we drained in reverse
            resolved_indices.reverse();
            resolved_data.reverse();

            let results_indices = std::mem::take(&mut results.indices);
            results.indices = resolved_indices
                .into_iter()
                .chain(results_indices)
                .collect();

            let resolved_data = FlatVecDeque::from_vecs(resolved_data);
            // Unwraps: the data is created when entering this function.
            let cached_dense = results
                .cached_dense
                .as_mut()
                .unwrap()
                .as_any_mut()
                .downcast_mut::<FlatVecDeque<C>>()
                .unwrap();
            cached_dense.push_front_deque(resolved_data);
        }

        if !results.promises_back.is_empty() {
            let mut resolved_indices = Vec::with_capacity(results.promises_back.len());
            let mut resolved_data = Vec::with_capacity(results.promises_back.len());

            for (index, promise) in results.promises_back.drain(..) {
                let data = match resolver.resolve(&promise) {
                    PromiseResult::Pending => {
                        unreachable!();
                        // TODO: stop there, we'll try again from here next time around
                        break;
                    }
                    // TODO: for now i think this is fine.. but in the future maybe we want to
                    // "ignore" this one and go on?
                    PromiseResult::Error(err) => {
                        // TODO: warning data dropped
                        re_log::error!(%err);
                        continue;
                    }
                    PromiseResult::Ready(cell) => {
                        match cell
                            .try_to_native::<C>()
                            .map_err(|err| DeserializationError::DataCellError(err.to_string()))
                        {
                            Ok(data) => data,
                            Err(err) => {
                                // TODO: warning data dropped
                                re_log::error!(%err);
                                continue;
                            }
                        }
                    }
                };

                resolved_indices.push(index);
                resolved_data.push(data);
            }

            results.indices.extend(resolved_indices);

            let resolved_data = FlatVecDeque::from_vecs(resolved_data);
            // Unwraps: the data is created when entering this function.
            let cached_dense = results
                .cached_dense
                .as_mut()
                .unwrap()
                .as_any_mut()
                .downcast_mut::<FlatVecDeque<C>>()
                .unwrap();
            cached_dense.push_back_deque(resolved_data);
        }

        // --- Step 2: fetch cached data (read lock) ---

        let results = RwLockWriteGuard::downgrade(results);

        CachedRangeData {
            // TODO: i need two mapped things because of the same parking_lot limitation :/
            indices: RwLockReadGuard::map(results, |results| &results.indices),
            data: RwLockReadGuard::map(self.0.read_recursive(), |results| {
                // Unwraps: the data is created when entering this function.
                results
                    .cached_dense
                    .as_ref()
                    .unwrap()
                    .as_any()
                    .downcast_ref::<FlatVecDeque<C>>()
                    .unwrap()
            }),
        }
    }

    // TODO(Amanieu/parking_lot#289): we cannot yet express the following because of a limitation
    // in `parking_lot`.
    // See <https://github.com/Amanieu/parking_lot/issues/289#issuecomment-1827545967>.
    //
    // /// Iterates over the component data, assuming it is dense.
    // ///
    // /// Returns an error if the component is missing or cannot be deserialized.
    // ///
    // /// Use [`PromiseResult::flatten`] to merge the results of resolving the promise and of
    // /// deserializing the data into a single one, if you don't need the extra flexibility.
    // #[inline]
    // pub fn iter_dense<C: Component>(
    //     &self,
    //     resolver: &PromiseResolver,
    // ) -> PromiseResult<crate::Result<MappedRwLockReadGuard<'_, impl Iterator<Item = &[C]>>>> {
    //     self.to_dense(&resolver)
    //         .map(|res| res.map(|data| MappedRwLockReadGuard::map(data, |data| data.iter())))
    // }

    /// Returns the component data as a sparse vector.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    ///
    /// Use [`PromiseResult::flatten`] to merge the results of resolving the promise and of
    /// deserializing the data into a single one, if you don't need the extra flexibility.
    //
    // TODO: this is literally the same code with Option<C> instead -- surely we can do better
    //
    // TODO: rm debug clause
    #[inline]
    pub fn to_sparse<C: Component + std::fmt::Debug>(
        &self,
        resolver: &PromiseResolver,
    ) -> CachedRangeData<'_, Option<C>> {
        // TODO: probably best to make sure we always hold both at once here.

        // --- Step 1: upsert pending data (write lock) ---

        // TODO: should probably be a try_write for the same reasons as before?
        let mut results = self.0.write();

        if results.cached_sparse.is_none() {
            results.cached_sparse = Some(Box::new(FlatVecDeque::<Option<C>>::new()));
        }

        if !results.promises_front.is_empty() {
            let mut resolved_indices = Vec::with_capacity(results.promises_front.len());
            let mut resolved_data = Vec::with_capacity(results.promises_front.len());

            // TODO: this is broken -- drain will drop everything if we stop halfway

            // TODO: we walk in reverse so that we can break on pending at the edge
            for (index, promise) in results.promises_front.drain(..).rev() {
                let data = match resolver.resolve(&promise) {
                    PromiseResult::Pending => {
                        unreachable!();
                        // TODO: stop there, we'll try again from here next time around
                        break;
                    }
                    // TODO: for now i think this is fine.. but in the future maybe we want to
                    // "ignore" this one and go on?
                    PromiseResult::Error(err) => {
                        // TODO: warning data dropped
                        re_log::error!(%err);
                        continue;
                    }
                    PromiseResult::Ready(cell) => {
                        match cell
                            .try_to_native_opt::<C>()
                            .map_err(|err| DeserializationError::DataCellError(err.to_string()))
                        {
                            Ok(data) => data,
                            Err(err) => {
                                // TODO: warning data dropped
                                re_log::error!(%err);
                                continue;
                            }
                        }
                    }
                };

                resolved_indices.push(index);
                resolved_data.push(data);
            }

            // TODO: uh-oh!! we can't be updating indices here and have indices() be its own method!!

            // TODO: because we drained in reverse
            resolved_indices.reverse();
            resolved_data.reverse();

            let results_indices = std::mem::take(&mut results.indices);
            results.indices = resolved_indices
                .into_iter()
                .chain(results_indices)
                .collect();

            let resolved_data = FlatVecDeque::from_vecs(resolved_data);
            // Unwraps: the data is created when entering this function.
            let cached_sparse = results
                .cached_sparse
                .as_mut()
                .unwrap()
                .as_any_mut()
                .downcast_mut::<FlatVecDeque<Option<C>>>()
                .unwrap();
            cached_sparse.push_front_deque(resolved_data);
        }

        if !results.promises_back.is_empty() {
            let mut resolved_indices = Vec::with_capacity(results.promises_back.len());
            let mut resolved_data = Vec::with_capacity(results.promises_back.len());

            for (index, promise) in results.promises_back.drain(..) {
                let data = match resolver.resolve(&promise) {
                    PromiseResult::Pending => {
                        unreachable!();
                        // TODO: stop there, we'll try again from here next time around
                        break;
                    }
                    // TODO: for now i think this is fine.. but in the future maybe we want to
                    // "ignore" this one and go on?
                    PromiseResult::Error(err) => {
                        // TODO: warning data dropped
                        re_log::error!(%err);
                        continue;
                    }
                    PromiseResult::Ready(cell) => {
                        match cell
                            .try_to_native_opt::<C>()
                            .map_err(|err| DeserializationError::DataCellError(err.to_string()))
                        {
                            Ok(data) => data,
                            Err(err) => {
                                // TODO: warning data dropped
                                re_log::error!(%err);
                                continue;
                            }
                        }
                    }
                };

                resolved_indices.push(index);
                resolved_data.push(data);
            }

            results.indices.extend(resolved_indices);

            let resolved_data = FlatVecDeque::from_vecs(resolved_data);
            // Unwraps: the data is created when entering this function.
            let cached_sparse = results
                .cached_sparse
                .as_mut()
                .unwrap()
                .as_any_mut()
                .downcast_mut::<FlatVecDeque<Option<C>>>()
                .unwrap();
            cached_sparse.push_back_deque(resolved_data);
        }

        // --- Step 2: fetch cached data (read lock) ---

        let results = RwLockWriteGuard::downgrade(results);

        CachedRangeData {
            // TODO: i need two mapped things because of the same parking_lot limitation :/
            indices: RwLockReadGuard::map(results, |results| &results.indices),
            data: RwLockReadGuard::map(self.0.read_recursive(), |results| {
                // Unwraps: the data is created when entering this function.
                results
                    .cached_sparse
                    .as_ref()
                    .unwrap()
                    .as_any()
                    .downcast_ref::<FlatVecDeque<Option<C>>>()
                    .unwrap()
            }),
        }
    }

    // TODO(Amanieu/parking_lot#289): we cannot yet express the following because of a limitation
    // in `parking_lot`.
    // See <https://github.com/Amanieu/parking_lot/issues/289#issuecomment-1827545967>.
    //
    // /// Iterates over the component data, assuming it is sparse.
    // ///
    // /// Returns an error if the component is missing or cannot be deserialized.
    // ///
    // /// Use [`PromiseResult::flatten`] to merge the results of resolving the promise and of
    // /// deserializing the data into a single one, if you don't need the extra flexibility.
    // #[inline]
    // pub fn iter_sparse<C: Component>(
    //     &self,
    //     resolver: &PromiseResolver,
    // ) -> PromiseResult<crate::Result<MappedRwLockReadGuard<'_, impl Iterator<Item = &[Option<C>]>>>>
    // {
    //     self.to_sparse(&resolver)
    //         .map(|res| res.map(|data| MappedRwLockReadGuard::map(data, |data| data.iter())))
    // }
}
