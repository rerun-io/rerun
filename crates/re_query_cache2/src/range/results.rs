use std::{
    cell::RefCell,
    collections::VecDeque,
    ops::Range,
    sync::{Arc, OnceLock},
};

use nohash_hasher::IntMap;

use parking_lot::{MappedRwLockReadGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};
use re_log_types::{RowId, TimeInt, TimeRange};
use re_types_core::{Component, ComponentName, DeserializationError, SizeBytes};

use crate::{ErasedFlatVecDeque, FlatVecDeque, Promise, PromiseResolver, PromiseResult};

// ---

/// Cached results for a range query.
///
/// The data is both deserialized and resolved/converted.
///
/// Use [`CachedRangeResults::get`], [`CachedRangeResults::get_required`] and
/// [`CachedRangeResults::get_or_empty`] in order to access the results for each individual component.
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
    pub fn get_or_empty(
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

/// Lazily cached results for a particular component when using a cached range query.
#[derive(Debug, Clone)]
pub struct CachedRangeComponentResults(Arc<RwLock<CachedRangeComponentResultsInner>>);

impl re_types_core::SizeBytes for CachedRangeComponentResults {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // NOTE: it's all on the heap past this point.
        self.0.read_recursive().total_size_bytes()
    }
}

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

pub struct CachedRangeData<'a, T> {
    // TODO(Amanieu/parking_lot#289): we need two distinct mapped guards because it's
    // impossible to return an owned type in a `parking_lot` guard.
    // See <https://github.com/Amanieu/parking_lot/issues/289#issuecomment-1827545967>.
    indices: MappedRwLockReadGuard<'a, VecDeque<(TimeInt, RowId)>>,
    data: MappedRwLockReadGuard<'a, FlatVecDeque<T>>,

    front_status: (TimeInt, PromiseResult<()>),
    back_status: (TimeInt, PromiseResult<()>),

    /// Keeps track of reentrancy counts for the current thread.
    ///
    /// Used to detect and prevent potential deadlocks when using the cached APIs in work-stealing
    /// environments such as Rayon.
    reentering: &'static std::thread::LocalKey<RefCell<u32>>,
}

impl<'a, T> Drop for CachedRangeData<'a, T> {
    #[inline]
    fn drop(&mut self) {
        self.reentering
            .with_borrow_mut(|reentering| *reentering = reentering.saturating_sub(1));
    }
}

impl<'a, T> CachedRangeData<'a, T> {
    /// Returns the current status on both ends of the range.
    ///
    /// E.g. it is possible that the front-side of the range is still waiting for pending data while
    /// the back-side has been fully loaded.
    #[inline]
    pub fn status(&self, time_range: TimeRange) -> (PromiseResult<()>, PromiseResult<()>) {
        let (front_time, front_status) = &self.front_status;
        let front_status = if *front_time >= time_range.min() {
            front_status.clone()
        } else {
            PromiseResult::Ready(())
        };

        let (back_time, back_status) = &self.back_status;
        let back_status = if *back_time <= time_range.min() {
            back_status.clone()
        } else {
            PromiseResult::Ready(())
        };

        (front_status, back_status)
    }

    #[inline]
    pub fn range_indices(
        &self,
        entry_range: Range<usize>,
    ) -> impl Iterator<Item = &(TimeInt, RowId)> {
        self.indices.range(entry_range)
    }

    #[inline]
    pub fn range_data(&self, entry_range: Range<usize>) -> impl Iterator<Item = &[T]> {
        self.data.range(entry_range)
    }

    /// Range both the indices and data by zipping them together.
    ///
    /// Useful for time-based joins (`range_zip`).
    #[inline]
    pub fn range_indexed(
        &self,
        time_range: TimeRange,
    ) -> impl Iterator<Item = (&(TimeInt, RowId), &[T])> {
        let entry_range = self.entry_range(time_range);
        itertools::izip!(
            self.range_indices(entry_range.clone()),
            self.range_data(entry_range)
        )
    }

    /// Returns the index range that corresponds to the specified `time_range`.
    ///
    /// Use the returned range with one of the range iteration methods:
    /// - [`Self::indices`]
    /// - [`Self::data`]
    /// - [`Self::range_indexed`]
    ///
    /// Make sure that the bucket hasn't been modified in-between!
    ///
    /// This is `O(2*log(n))`, so make sure to clone the returned range rather than calling this
    /// multiple times.
    #[inline]
    pub fn entry_range(&self, time_range: TimeRange) -> Range<usize> {
        // If there's any static data cached, make sure to look for it explicitly.
        //
        // Remember: time ranges can never contain `TimeInt::STATIC`.
        let static_override = if matches!(self.indices.front(), Some((TimeInt::STATIC, _))) {
            TimeInt::STATIC
        } else {
            TimeInt::MAX
        };

        let start_index = self.indices.partition_point(|(data_time, _)| {
            *data_time < TimeInt::min(time_range.min(), static_override)
        });
        let end_index = self.indices.partition_point(|(data_time, _)| {
            *data_time <= TimeInt::min(time_range.max(), static_override)
        });

        start_index..end_index
    }
}

impl CachedRangeComponentResults {
    /// Returns the component data as a dense vector.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    ///
    /// Use [`PromiseResult::flatten`] to merge the results of resolving the promise and of
    /// deserializing the data into a single one, if you don't need the extra flexibility.
    #[inline]
    pub fn to_dense<C: Component>(&self, resolver: &PromiseResolver) -> CachedRangeData<'_, C> {
        // --- Step 1: try and upsert pending data (write lock) ---

        thread_local! {
            /// Keeps track of reentrancy counts for the current thread.
            ///
            /// Used to detect and prevent potential deadlocks when using the cached APIs in work-stealing
            /// environments such as Rayon.
            static REENTERING: RefCell<u32> = const { RefCell::new(0) };
        }

        REENTERING.with_borrow_mut(|reentering| *reentering = reentering.saturating_add(1));

        let mut results = if let Some(results) = self.0.try_write() {
            // The lock was free to grab, nothing else to worry about.
            Some(results)
        } else {
            REENTERING.with_borrow_mut(|reentering| {
                if *reentering > 1 {
                    // The lock is busy, and at least one of the lock holders is the current thread from a
                    // previous stack frame.
                    //
                    // Return `None` so that we skip straight to the read-only part of the operation.
                    // All the data will be there already, since the previous stack frame already
                    // took care of upserting it.
                    None
                } else {
                    // The lock is busy, but it is not held by the current thread.
                    // Just block untils it gets released.
                    Some(self.0.write())
                }
            })
        };

        if let Some(results) = &mut results {
            // NOTE: This is just a lazy initialization of the underlying deque, because we
            // just now finally know the expected type!
            if results.cached_dense.is_none() {
                results.cached_dense = Some(Box::new(FlatVecDeque::<C>::new()));
            }

            if results.cached_sparse.is_some() {
                re_log::error!(
                    "a component cannot be both dense and sparse -- try `to_sparse()` instead"
                );
            } else {
                if !results.promises_front.is_empty() {
                    let mut resolved_indices = Vec::with_capacity(results.promises_front.len());
                    let mut resolved_data = Vec::with_capacity(results.promises_front.len());

                    // Pop the promises from the end so that if we encounter one that has yet to be
                    // resolved, we can stop right there and know we have a contiguous range of data
                    // available up to that point in time.
                    while let Some(((data_time, row_id), promise)) = results.promises_front.pop() {
                        let data = match resolver.resolve(&promise) {
                            PromiseResult::Pending => {
                                results.front_status = (data_time, PromiseResult::Pending);
                                break;
                            }
                            PromiseResult::Error(err) => {
                                results.front_status = (data_time, PromiseResult::Error(err));
                                break;
                            }
                            PromiseResult::Ready(cell) => {
                                results.front_status = (data_time, PromiseResult::Ready(()));
                                match cell.try_to_native::<C>().map_err(|err| {
                                    DeserializationError::DataCellError(err.to_string())
                                }) {
                                    Ok(data) => data,
                                    Err(err) => {
                                        re_log::error!(%err, component=%C::name(), "data deserialization failed -- skipping");
                                        continue;
                                    }
                                }
                            }
                        };

                        resolved_indices.push((data_time, row_id));
                        resolved_data.push(data);
                    }

                    // We resolved the promises in reversed order, so reverse the results back.
                    resolved_indices.reverse();
                    resolved_data.reverse();

                    let results_indices = std::mem::take(&mut results.indices);
                    results.indices = resolved_indices
                        .into_iter()
                        .chain(results_indices)
                        .collect();

                    let resolved_data = FlatVecDeque::from_vecs(resolved_data);
                    // Unwraps: the data is created when entering this function -- we know it's there
                    // and we know its type.
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

                    // Reverse the promises first so we can pop() from the back. See below why.
                    results.promises_back.reverse();

                    // Pop the promises from the end so that if we encounter one that has yet to be
                    // resolved, we can stop right there and know we have a contiguous range of data
                    // available up to that point in time.
                    while let Some(((data_time, index), promise)) = results.promises_back.pop() {
                        let data = match resolver.resolve(&promise) {
                            PromiseResult::Pending => {
                                results.back_status = (data_time, PromiseResult::Pending);
                                break;
                            }
                            PromiseResult::Error(err) => {
                                results.back_status = (data_time, PromiseResult::Error(err));
                                break;
                            }
                            PromiseResult::Ready(cell) => {
                                results.front_status = (data_time, PromiseResult::Ready(()));
                                match cell.try_to_native::<C>().map_err(|err| {
                                    DeserializationError::DataCellError(err.to_string())
                                }) {
                                    Ok(data) => data,
                                    Err(err) => {
                                        re_log::error!(%err, "data deserialization failed -- skipping");
                                        continue;
                                    }
                                }
                            }
                        };

                        resolved_indices.push((data_time, index));
                        resolved_data.push(data);
                    }

                    // Reverse our reversal and give the promises back to their rightful owner.
                    results.promises_back.reverse();

                    results.indices.extend(resolved_indices);

                    let resolved_data = FlatVecDeque::from_vecs(resolved_data);
                    // Unwraps: the data is created when entering this function -- we know it's there
                    // and we know its type.
                    let cached_dense = results
                        .cached_dense
                        .as_mut()
                        .unwrap()
                        .as_any_mut()
                        .downcast_mut::<FlatVecDeque<C>>()
                        .unwrap();
                    cached_dense.push_back_deque(resolved_data);
                }

                results.sanity_check();
            }
        }

        // --- Step 2: fetch cached data (read lock) ---

        let results = if let Some(results) = results {
            RwLockWriteGuard::downgrade(results)
        } else {
            // # Multithreading semantics
            //
            // We need the reentrant lock because query contexts (i.e. space views) generally run on a
            // work-stealing thread-pool and might swap a task on one thread with another task on the
            // same thread, where both tasks happen to query the same exact data (e.g. cloned space views).
            //
            // See `REENTERING` comments above for more details.
            self.read_recursive()
        };

        let front_status = results.front_status.clone();
        let back_status = results.back_status.clone();

        // TODO(Amanieu/parking_lot#289): we need two distinct mapped guards because it's
        // impossible to return an owned type in a `parking_lot` guard.
        // See <https://github.com/Amanieu/parking_lot/issues/289#issuecomment-1827545967>.
        let indices = RwLockReadGuard::map(results, |results| &results.indices);
        let data = RwLockReadGuard::map(self.0.read_recursive(), |results| {
            // Unwraps: the data is created when entering this function -- we know it's there
            // and we know its type.
            results
                .cached_dense
                .as_ref()
                .unwrap()
                .as_any()
                .downcast_ref::<FlatVecDeque<C>>()
                .unwrap()
        });

        CachedRangeData {
            indices,
            data,
            front_status,
            back_status,
            reentering: &REENTERING,
        }
    }

    /// Returns the component data as a sparse vector.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    ///
    /// Use [`PromiseResult::flatten`] to merge the results of resolving the promise and of
    /// deserializing the data into a single one, if you don't need the extra flexibility.
    //
    // TODO(cmc): this is _almost_ a byte-for-byte copy of the `to_dense` case but those few bits
    // that differ cannot be sanely abstracted over with today's Rust…
    #[inline]
    pub fn to_sparse<C: Component>(
        &self,
        resolver: &PromiseResolver,
    ) -> CachedRangeData<'_, Option<C>> {
        // --- Step 1: try and upsert pending data (write lock) ---

        thread_local! {
            /// Keeps track of reentrancy counts for the current thread.
            ///
            /// Used to detect and prevent potential deadlocks when using the cached APIs in work-stealing
            /// environments such as Rayon.
            static REENTERING: RefCell<u32> = const { RefCell::new(0) };
        }

        REENTERING.with_borrow_mut(|reentering| *reentering = reentering.saturating_add(1));

        let mut results = if let Some(results) = self.0.try_write() {
            // The lock was free to grab, nothing else to worry about.
            Some(results)
        } else {
            REENTERING.with_borrow_mut(|reentering| {
                if *reentering > 1 {
                    // The lock is busy, and at least one of the lock holders is the current thread from a
                    // previous stack frame.
                    //
                    // Return `None` so that we skip straight to the read-only part of the operation.
                    // All the data will be there already, since the previous stack frame already
                    // took care of upserting it.
                    None
                } else {
                    // The lock is busy, but it is not held by the current thread.
                    // Just block untils it gets released.
                    Some(self.0.write())
                }
            })
        };

        if let Some(results) = &mut results {
            // NOTE: This is just a lazy initialization of the underlying deque, because we
            // just now finally know the expected type!
            if results.cached_sparse.is_none() {
                results.cached_sparse = Some(Box::new(FlatVecDeque::<Option<C>>::new()));
            }

            if results.cached_dense.is_some() {
                re_log::error!(
                    "a component cannot be both dense and sparse -- try `to_dense()` instead"
                );
            } else {
                if !results.promises_front.is_empty() {
                    let mut resolved_indices = Vec::with_capacity(results.promises_front.len());
                    let mut resolved_data = Vec::with_capacity(results.promises_front.len());

                    // Pop the promises from the end so that if we encounter one that has yet to be
                    // resolved, we can stop right there and know we have a contiguous range of data
                    // available up to that point in time.
                    while let Some(((data_time, row_id), promise)) = results.promises_front.pop() {
                        let data = match resolver.resolve(&promise) {
                            PromiseResult::Pending => {
                                results.front_status = (data_time, PromiseResult::Pending);
                                break;
                            }
                            PromiseResult::Error(err) => {
                                results.front_status = (data_time, PromiseResult::Error(err));
                                break;
                            }
                            PromiseResult::Ready(cell) => {
                                results.front_status = (data_time, PromiseResult::Ready(()));
                                match cell.try_to_native_opt::<C>().map_err(|err| {
                                    DeserializationError::DataCellError(err.to_string())
                                }) {
                                    Ok(data) => data,
                                    Err(err) => {
                                        re_log::error!(%err, component=%C::name(), "data deserialization failed -- skipping");
                                        continue;
                                    }
                                }
                            }
                        };

                        resolved_indices.push((data_time, row_id));
                        resolved_data.push(data);
                    }

                    // We resolved the promises in reversed order, so reverse the results back.
                    resolved_indices.reverse();
                    resolved_data.reverse();

                    let results_indices = std::mem::take(&mut results.indices);
                    results.indices = resolved_indices
                        .into_iter()
                        .chain(results_indices)
                        .collect();

                    let resolved_data = FlatVecDeque::from_vecs(resolved_data);
                    // Unwraps: the data is created when entering this function -- we know it's there
                    // and we know its type.
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

                    // Reverse the promises first so we can pop() from the back. See below why.
                    results.promises_back.reverse();

                    // Pop the promises from the end so that if we encounter one that has yet to be
                    // resolved, we can stop right there and know we have a contiguous range of data
                    // available up to that point in time.
                    while let Some(((data_time, index), promise)) = results.promises_back.pop() {
                        let data = match resolver.resolve(&promise) {
                            PromiseResult::Pending => {
                                results.back_status = (data_time, PromiseResult::Pending);
                                break;
                            }
                            PromiseResult::Error(err) => {
                                results.back_status = (data_time, PromiseResult::Error(err));
                                break;
                            }
                            PromiseResult::Ready(cell) => {
                                results.front_status = (data_time, PromiseResult::Ready(()));
                                match cell.try_to_native_opt::<C>().map_err(|err| {
                                    DeserializationError::DataCellError(err.to_string())
                                }) {
                                    Ok(data) => data,
                                    Err(err) => {
                                        re_log::error!(%err, "data deserialization failed -- skipping");
                                        continue;
                                    }
                                }
                            }
                        };

                        resolved_indices.push((data_time, index));
                        resolved_data.push(data);
                    }

                    // Reverse our reversal and give the promises back to their rightful owner.
                    results.promises_back.reverse();

                    results.indices.extend(resolved_indices);

                    let resolved_data = FlatVecDeque::from_vecs(resolved_data);
                    // Unwraps: the data is created when entering this function -- we know it's there
                    // and we know its type.
                    let cached_sparse = results
                        .cached_sparse
                        .as_mut()
                        .unwrap()
                        .as_any_mut()
                        .downcast_mut::<FlatVecDeque<Option<C>>>()
                        .unwrap();
                    cached_sparse.push_back_deque(resolved_data);
                }

                results.sanity_check();
            }
        }

        // --- Step 2: fetch cached data (read lock) ---

        let results = if let Some(results) = results {
            RwLockWriteGuard::downgrade(results)
        } else {
            // # Multithreading semantics
            //
            // We need the reentrant lock because query contexts (i.e. space views) generally run on a
            // work-stealing thread-pool and might swap a task on one thread with another task on the
            // same thread, where both tasks happen to query the same exact data (e.g. cloned space views).
            //
            // See `REENTERING` comments above for more details.
            self.read_recursive()
        };

        let front_status = results.front_status.clone();
        let back_status = results.back_status.clone();

        // TODO(Amanieu/parking_lot#289): we need two distinct mapped guards because it's
        // impossible to return an owned type in a `parking_lot` guard.
        // See <https://github.com/Amanieu/parking_lot/issues/289#issuecomment-1827545967>.
        let indices = RwLockReadGuard::map(results, |results| &results.indices);
        let data = RwLockReadGuard::map(self.0.read_recursive(), |results| {
            // Unwraps: the data is created when entering this function -- we know it's there
            // and we know its type.
            results
                .cached_sparse
                .as_ref()
                .unwrap()
                .as_any()
                .downcast_ref::<FlatVecDeque<Option<C>>>()
                .unwrap()
        });

        CachedRangeData {
            indices,
            data,
            front_status,
            back_status,
            reentering: &REENTERING,
        }
    }
}

// ---

/// Lazily cached results for a particular component when using a cached range query.
pub struct CachedRangeComponentResultsInner {
    pub(crate) indices: VecDeque<(TimeInt, RowId)>,

    /// All the pending promises that must resolved in order to fill the missing data on the
    /// front-side of the ringbuffer (i.e. further back in time).
    ///
    /// Always sorted in ascending index order ([`TimeInt`] + [`RowId`] pair).
    pub(crate) promises_front: Vec<((TimeInt, RowId), Promise)>,

    /// All the pending promises that must resolved in order to fill the missing data on the
    /// back-side of the ringbuffer (i.e. the most recent data).
    ///
    /// Always sorted in ascending index order ([`TimeInt`] + [`RowId`] pair).
    pub(crate) promises_back: Vec<((TimeInt, RowId), Promise)>,

    /// Keeps track of the status of the data on the front-side of the cache.
    pub(crate) front_status: (TimeInt, PromiseResult<()>),

    /// Keeps track of the status of the data on the back-side of the cache.
    pub(crate) back_status: (TimeInt, PromiseResult<()>),

    /// The resolved, converted, deserialized dense data.
    ///
    /// This has to be option because we have no way of initializing the underlying trait object
    /// until we know what the actual native type that the caller expects is.
    ///
    /// Once `cached_dense` has been initialized, it is an error to try and use the sparse methods.
    pub(crate) cached_dense: Option<Box<dyn ErasedFlatVecDeque + Send + Sync>>,

    /// The resolved, converted, deserialized sparse data.
    ///
    /// This has to be option because we have no way of initializing the underlying trait object
    /// until we know what the actual native type that the caller expects is.
    ///
    /// Once `cached_sparse` has been initialized, it is an error to try and use the dense methods.
    pub(crate) cached_sparse: Option<Box<dyn ErasedFlatVecDeque + Send + Sync>>,
}

impl SizeBytes for CachedRangeComponentResultsInner {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            indices,
            promises_front,
            promises_back,
            front_status: _,
            back_status: _,
            cached_dense,
            cached_sparse,
        } = self;

        indices.heap_size_bytes()
            + promises_front.heap_size_bytes()
            + promises_back.heap_size_bytes()
            + cached_dense
                .as_ref()
                .map_or(0, |data| data.dyn_total_size_bytes())
            + cached_sparse
                .as_ref()
                .map_or(0, |data| data.dyn_total_size_bytes())
    }
}

impl std::fmt::Debug for CachedRangeComponentResultsInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            indices,
            promises_front: _,
            promises_back: _,
            front_status: _,
            back_status: _,
            cached_dense: _,  // we can't, we don't know the type
            cached_sparse: _, // we can't, we don't know the type
        } = self;

        if indices.is_empty() {
            f.write_str("<empty>")
        } else {
            // Unwrap: checked above.
            let index_start = indices.front().unwrap();
            let index_end = indices.back().unwrap();
            f.write_fmt(format_args!(
                "[{:?}#{} .. {:?}#{}] {}",
                index_start.0,
                index_start.1,
                index_end.0,
                index_end.1,
                re_format::format_bytes(self.total_size_bytes() as _)
            ))
        }
    }
}

impl CachedRangeComponentResultsInner {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            indices: VecDeque::new(),
            promises_front: Vec::new(),
            promises_back: Vec::new(),
            front_status: (TimeInt::MIN, PromiseResult::Ready(())),
            back_status: (TimeInt::MAX, PromiseResult::Ready(())),
            cached_dense: None,
            cached_sparse: None,
        }
    }

    /// No-op in release.
    #[inline]
    pub fn sanity_check(&self) {
        let Self {
            indices,
            promises_front,
            promises_back,
            front_status: _,
            back_status: _,
            cached_dense,
            cached_sparse,
        } = self;

        assert!(
            promises_front.windows(2).all(|promises| {
                let index_left = promises[0].0;
                let index_right = promises[1].0;
                index_left <= index_right
            }),
            "front promises must always be sorted in ascending index order"
        );
        if let (Some(p_index), Some(i_index)) = (
            promises_front.last().map(|(index, _)| index),
            indices.front(),
        ) {
            assert!(
                p_index < i_index,
                "the rightmost front promise must have an index smaller than the leftmost data index ({p_index:?} < {i_index:?})",
            );
        }

        assert!(
            promises_back.windows(2).all(|promises| {
                let index_left = promises[0].0;
                let index_right = promises[1].0;
                index_left <= index_right
            }),
            "back promises must always be sorted in ascending index order"
        );
        if let (Some(p_index), Some(i_index)) =
            (promises_back.last().map(|(index, _)| index), indices.back())
        {
            assert!(
                i_index < p_index,
                "the leftmost back promise must have an index larger than the rightmost data index ({i_index:?} < {p_index:?})",
            );
        }

        if let Some(dense) = cached_dense.as_ref() {
            assert_eq!(indices.len(), dense.dyn_num_entries());
        }

        if let Some(sparse) = cached_sparse.as_ref() {
            assert_eq!(indices.len(), sparse.dyn_num_entries());
        }
    }

    /// Returns the time range covered by the cached data.
    ///
    /// Reminder: [`TimeInt::STATIC`] is never included in [`TimeRange`]s.
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

    /// Removes everything from the bucket that corresponds to a time equal or greater than the
    /// specified `threshold`.
    ///
    /// Returns the number of bytes removed.
    #[inline]
    pub fn truncate_at_time(&mut self, threshold: TimeInt) {
        re_tracing::profile_function!();

        let time_range = self.time_range();

        let Self {
            indices,
            promises_front,
            promises_back,
            front_status,
            back_status,
            cached_dense,
            cached_sparse,
        } = self;

        if front_status.0 >= threshold {
            let time_min = time_range.map_or(TimeInt::MIN, |range| range.min());
            *front_status = (time_min, PromiseResult::Ready(()));
        }
        if back_status.0 >= threshold {
            let time_max = time_range.map_or(TimeInt::MAX, |range| range.max());
            *back_status = (time_max, PromiseResult::Ready(()));
        }

        // NOTE: promises are kept ascendingly sorted by index
        {
            let threshold_idx =
                promises_front.partition_point(|((data_time, _), _)| *data_time < threshold);
            promises_front.truncate(threshold_idx);

            let threshold_idx =
                promises_back.partition_point(|((data_time, _), _)| *data_time < threshold);
            promises_back.truncate(threshold_idx);
        }

        let threshold_idx = indices.partition_point(|(data_time, _)| data_time < &threshold);
        {
            indices.truncate(threshold_idx);
            if let Some(data) = cached_dense {
                data.dyn_truncate(threshold_idx);
            }
            if let Some(data) = cached_sparse {
                data.dyn_truncate(threshold_idx);
            }
        }

        self.sanity_check();
    }

    #[inline]
    pub fn clear(&mut self) {
        *self = Self::empty();
    }
}
