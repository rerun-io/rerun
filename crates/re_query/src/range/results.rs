use std::{
    cell::RefCell,
    collections::VecDeque,
    ops::Range,
    sync::{Arc, OnceLock},
};

use nohash_hasher::IntMap;

use parking_lot::{MappedRwLockReadGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};
use re_data_store::RangeQuery;
use re_log_types::{ResolvedTimeRange, RowId, TimeInt};
use re_types_core::{Component, ComponentName, DeserializationError, SizeBytes};

use crate::{
    ErasedFlatVecDeque, FlatVecDeque, LatestAtComponentResults, Promise, PromiseResolver,
    PromiseResult,
};

// ---

/// Results for a range query.
///
/// The data is both deserialized and resolved/converted.
///
/// Use [`RangeResults::get`], [`RangeResults::get_required`] and
/// [`RangeResults::get_or_empty`] in order to access the results for each individual component.
#[derive(Debug)]
pub struct RangeResults {
    pub query: RangeQuery,
    pub components: IntMap<ComponentName, RangeComponentResults>,
}

impl RangeResults {
    #[inline]
    pub(crate) fn new(query: RangeQuery) -> Self {
        Self {
            query,
            components: Default::default(),
        }
    }

    #[inline]
    pub fn contains(&self, component_name: impl Into<ComponentName>) -> bool {
        self.components.contains_key(&component_name.into())
    }

    /// Returns the [`RangeComponentResults`] for the specified [`Component`].
    #[inline]
    pub fn get(&self, component_name: impl Into<ComponentName>) -> Option<&RangeComponentResults> {
        self.components.get(&component_name.into())
    }

    /// Returns the [`RangeComponentResults`] for the specified [`Component`].
    ///
    /// Returns an error if the component is not present.
    #[inline]
    pub fn get_required(
        &self,
        component_name: impl Into<ComponentName>,
    ) -> crate::Result<&RangeComponentResults> {
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

    /// Returns the [`RangeComponentResults`] for the specified [`Component`].
    ///
    /// Returns empty results if the component is not present.
    #[inline]
    pub fn get_or_empty(&self, component_name: impl Into<ComponentName>) -> &RangeComponentResults {
        let component_name = component_name.into();
        if let Some(component) = self.components.get(&component_name) {
            component
        } else {
            RangeComponentResults::empty()
        }
    }
}

impl RangeResults {
    #[doc(hidden)]
    #[inline]
    pub fn add(&mut self, component_name: ComponentName, cached: RangeComponentResults) {
        self.components.insert(component_name, cached);
    }
}

// ---

thread_local! {
    /// Keeps track of reentrancy counts for the current thread.
    ///
    /// Used to detect and prevent potential deadlocks when using the cached APIs in work-stealing
    /// environments such as Rayon.
    static REENTERING: RefCell<u32> = const { RefCell::new(0) };
}

/// Lazily cached results for a particular component when using a cached range query.
#[derive(Debug)]
pub struct RangeComponentResults {
    /// The [`ResolvedTimeRange`] of the query that was used in order to retrieve these results in the
    /// first place.
    ///
    /// The "original" copy in the cache just stores [`ResolvedTimeRange::EMPTY`]. It's meaningless.
    pub(crate) time_range: ResolvedTimeRange,

    pub(crate) inner: Arc<RwLock<RangeComponentResultsInner>>,
}

impl RangeComponentResults {
    /// Clones the results while making sure to stamp them with the [`ResolvedTimeRange`] of the associated query.
    #[inline]
    pub(crate) fn clone_at(&self, time_range: ResolvedTimeRange) -> Self {
        Self {
            time_range,
            inner: self.inner.clone(),
        }
    }
}

impl RangeComponentResults {
    #[inline]
    pub fn empty() -> &'static Self {
        static EMPTY: OnceLock<RangeComponentResults> = OnceLock::new();
        EMPTY.get_or_init(Self::default)
    }
}

impl re_types_core::SizeBytes for RangeComponentResults {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // NOTE: it's all on the heap past this point.
        self.inner.read_recursive().total_size_bytes()
    }
}

impl Default for RangeComponentResults {
    #[inline]
    fn default() -> Self {
        Self {
            time_range: ResolvedTimeRange::EMPTY,
            inner: Arc::new(RwLock::new(RangeComponentResultsInner::empty())),
        }
    }
}

impl std::ops::Deref for RangeComponentResults {
    type Target = RwLock<RangeComponentResultsInner>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Helper datastructure to make it possible to convert latest-at results into ranged results.
#[derive(Debug)]
enum Indices<'a> {
    Owned(VecDeque<(TimeInt, RowId)>),
    Cached(MappedRwLockReadGuard<'a, VecDeque<(TimeInt, RowId)>>),
}

impl<'a> std::ops::Deref for Indices<'a> {
    type Target = VecDeque<(TimeInt, RowId)>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        match self {
            Indices::Owned(data) => data,
            Indices::Cached(data) => data,
        }
    }
}

/// Helper datastructure to make it possible to convert latest-at results into ranged results.
enum Data<'a, T> {
    Owned(Arc<dyn ErasedFlatVecDeque + Send + Sync>),
    Cached(MappedRwLockReadGuard<'a, FlatVecDeque<T>>),
}

impl<'a, T: 'static> std::ops::Deref for Data<'a, T> {
    type Target = FlatVecDeque<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        match self {
            Data::Owned(data) => {
                // Unwrap: only way to instantiate a `Data` is via the `From` impl below which we
                // fully control.
                data.as_any().downcast_ref().unwrap()
            }
            Data::Cached(data) => data,
        }
    }
}

pub struct RangeData<'a, T> {
    // NOTE: Options so we can represent an empty result without having to somehow conjure a mutex
    // guard out of thin air.
    //
    // TODO(Amanieu/parking_lot#289): we need two distinct mapped guards because it's
    // impossible to return an owned type in a `parking_lot` guard.
    // See <https://github.com/Amanieu/parking_lot/issues/289#issuecomment-1827545967>.
    // indices: Option<MappedRwLockReadGuard<'a, VecDeque<(TimeInt, RowId)>>>,
    indices: Option<Indices<'a>>,
    data: Option<Data<'a, T>>,

    time_range: ResolvedTimeRange,
    front_status: PromiseResult<()>,
    back_status: PromiseResult<()>,

    /// Keeps track of reentrancy counts for the current thread.
    ///
    /// Used to detect and prevent potential deadlocks when using the cached APIs in work-stealing
    /// environments such as Rayon.
    reentering: &'static std::thread::LocalKey<RefCell<u32>>,
}

impl<'a, C: Component> RangeData<'a, C> {
    /// Useful to abstract over latest-at and ranged results.
    ///
    /// Use `reindexed` override the index of the data, if needed.
    #[inline]
    pub fn from_latest_at(
        resolver: &PromiseResolver,
        results: &'a LatestAtComponentResults,
        reindexed: Option<(TimeInt, RowId)>,
    ) -> Self {
        let LatestAtComponentResults {
            index,
            promise: _,
            cached_dense,
        } = results;

        let status = results.to_dense::<C>(resolver).map(|_| ());
        let index = reindexed.unwrap_or(*index);

        Self {
            indices: Some(Indices::Owned(vec![index].into())),
            data: cached_dense.get().map(|data| Data::Owned(Arc::clone(data))),
            time_range: ResolvedTimeRange::new(index.0, index.0),
            front_status: status.clone(),
            back_status: status,
            reentering: &REENTERING,
        }
    }
}

impl<'a, T> Drop for RangeData<'a, T> {
    #[inline]
    fn drop(&mut self) {
        self.reentering
            .with_borrow_mut(|reentering| *reentering = reentering.saturating_sub(1));
    }
}

impl<'a, T: 'static> RangeData<'a, T> {
    /// Returns the current status on both ends of the range.
    ///
    /// E.g. it is possible that the front-side of the range is still waiting for pending data while
    /// the back-side has been fully loaded.
    #[inline]
    pub fn status(&self) -> (PromiseResult<()>, PromiseResult<()>) {
        (self.front_status.clone(), self.back_status.clone())
    }

    #[inline]
    pub fn range_indices(
        &self,
        entry_range: Range<usize>,
    ) -> impl Iterator<Item = &(TimeInt, RowId)> {
        let indices = match self.indices.as_ref() {
            Some(indices) => itertools::Either::Left(indices.range(entry_range)),
            None => itertools::Either::Right(std::iter::empty()),
        };
        indices
    }

    #[inline]
    pub fn range_data(&self, entry_range: Range<usize>) -> impl Iterator<Item = &[T]> {
        match self.data.as_ref() {
            Some(indices) => itertools::Either::Left(indices.range(entry_range)),
            None => itertools::Either::Right(std::iter::empty()),
        }
    }

    /// Range both the indices and data by zipping them together.
    ///
    /// Useful for time-based joins (`range_zip`).
    #[inline]
    pub fn range_indexed(&self) -> impl Iterator<Item = (&(TimeInt, RowId), &[T])> {
        let entry_range = self.entry_range();
        itertools::izip!(
            self.range_indices(entry_range.clone()),
            self.range_data(entry_range)
        )
    }

    /// Returns the index range that corresponds to the specified `time_range`.
    ///
    /// Use the returned range with one of the range iteration methods:
    /// - [`Self::range_indices`]
    /// - [`Self::range_data`]
    /// - [`Self::range_indexed`]
    ///
    /// Make sure that the bucket hasn't been modified in-between!
    ///
    /// This is `O(2*log(n))`, so make sure to clone the returned range rather than calling this
    /// multiple times.
    #[inline]
    pub fn entry_range(&self) -> Range<usize> {
        let Some(indices) = self.indices.as_ref() else {
            return 0..0;
        };

        // If there's any static data cached, make sure to look for it explicitly.
        //
        // Remember: `TimeRange`s can never contain `TimeInt::STATIC`.
        let static_override = if matches!(indices.front(), Some((TimeInt::STATIC, _))) {
            TimeInt::STATIC
        } else {
            TimeInt::MAX
        };

        let start_index = indices.partition_point(|(data_time, _)| {
            *data_time < TimeInt::min(self.time_range.min(), static_override)
        });
        let end_index = indices.partition_point(|(data_time, _)| {
            *data_time <= TimeInt::min(self.time_range.max(), static_override)
        });

        start_index..end_index
    }
}

impl RangeComponentResults {
    /// Returns the component data as a dense vector.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    ///
    /// Use [`PromiseResult::flatten`] to merge the results of resolving the promise and of
    /// deserializing the data into a single one, if you don't need the extra flexibility.
    #[inline]
    pub fn to_dense<C: Component>(&self, resolver: &PromiseResolver) -> RangeData<'_, C> {
        // It's tracing the deserialization of an entire range query at once -- it's fine.
        re_tracing::profile_function!();

        // --- Step 1: try and upsert pending data (write lock) ---

        REENTERING.with_borrow_mut(|reentering| *reentering = reentering.saturating_add(1));

        // Manufactured empty result.
        if self.time_range == ResolvedTimeRange::EMPTY {
            return RangeData {
                indices: None,
                data: None,
                time_range: ResolvedTimeRange::EMPTY,
                front_status: PromiseResult::Ready(()),
                back_status: PromiseResult::Ready(()),
                reentering: &REENTERING,
            };
        }

        let mut results = if let Some(results) = self.inner.try_write() {
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
                    // Just block until it gets released.
                    Some(self.inner.write())
                }
            })
        };

        if let Some(results) = &mut results {
            // NOTE: This is just a lazy initialization of the underlying deque, because we
            // just now finally know the expected type!
            if results.cached_dense.is_none() {
                results.cached_dense = Some(Box::new(FlatVecDeque::<C>::new()));
            }

            if !results.promises_front.is_empty() {
                re_tracing::profile_scope!("front");

                let mut resolved_indices = Vec::with_capacity(results.promises_front.len());
                let mut resolved_data = Vec::with_capacity(results.promises_front.len());

                // Pop the promises from the end so that if we encounter one that has yet to be
                // resolved, we can stop right there and know we have a contiguous range of data
                // available up to that point in time.
                //
                // Reminder: promises are sorted in ascending index order.
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
                            match cell
                                .try_to_native::<C>()
                                .map_err(|err| DeserializationError::DataCellError(err.to_string()))
                            {
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
                // Unwraps: the deque is created when entering this function -- we know it's there
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
                re_tracing::profile_scope!("back");

                let mut resolved_indices = Vec::with_capacity(results.promises_back.len());
                let mut resolved_data = Vec::with_capacity(results.promises_back.len());

                // Reverse the promises first so we can pop() from the back.
                // It's fine, this is a one-time operation in the successful case, and it's extremely fast to do.
                // See below why.
                //
                // Reminder: promises are sorted in ascending index order.
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
                            match cell
                                .try_to_native::<C>()
                                .map_err(|err| DeserializationError::DataCellError(err.to_string()))
                            {
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

                // Reverse our reversal.
                results.promises_back.reverse();

                results.indices.extend(resolved_indices);

                let resolved_data = FlatVecDeque::from_vecs(resolved_data);
                // Unwraps: the deque is created when entering this function -- we know it's there
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

        let front_status = {
            let (results_front_time, results_front_status) = &results.front_status;
            let query_front_time = self.time_range.min();
            if query_front_time < *results_front_time {
                // If the query covers a larger time span on its front-side than the resulting data, then
                // we should forward the status of the resulting data so the caller can know why it's
                // been cropped off.
                results_front_status.clone()
            } else {
                PromiseResult::Ready(())
            }
        };
        let back_status = {
            let (results_back_time, results_back_status) = &results.back_status;
            let query_back_time = self.time_range.max();
            if query_back_time > *results_back_time {
                // If the query covers a larger time span on its back-side than the resulting data, then
                // we should forward the status of the resulting data so the caller can know why it's
                // been cropped off.
                results_back_status.clone()
            } else {
                PromiseResult::Ready(())
            }
        };

        // # Reentrancy edge-case
        //
        // If we are in the reentrancy case, and if it's the first time this cache is used at all, and if
        // the previous stack-frame that was holding the lock finally decided to release it without
        // actually caching anything, then the deserialization cache still won't be initialized.
        //
        // Just leave it be for now, it'll fix itself by next frame.
        if results.cached_dense.is_none() {
            return RangeData {
                indices: None,
                data: None,
                time_range: ResolvedTimeRange::EMPTY,
                front_status: PromiseResult::Ready(()),
                back_status: PromiseResult::Ready(()),
                reentering: &REENTERING,
            };
        }

        // TODO(Amanieu/parking_lot#289): we need two distinct mapped guards because it's
        // impossible to return an owned type in a `parking_lot` guard.
        // See <https://github.com/Amanieu/parking_lot/issues/289#issuecomment-1827545967>.
        let indices = RwLockReadGuard::map(results, |results| &results.indices);
        let data = RwLockReadGuard::map(self.inner.read_recursive(), |results| {
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

        RangeData {
            indices: Some(Indices::Cached(indices)),
            data: Some(Data::Cached(data)),
            time_range: self.time_range,
            front_status,
            back_status,
            reentering: &REENTERING,
        }
    }
}

// ---

/// Lazily cached results for a particular component when using a cached range query.
pub struct RangeComponentResultsInner {
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
    pub(crate) cached_dense: Option<Box<dyn ErasedFlatVecDeque + Send + Sync>>,
}

impl Clone for RangeComponentResultsInner {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            indices: self.indices.clone(),
            promises_front: self.promises_front.clone(),
            promises_back: self.promises_back.clone(),
            front_status: self.front_status.clone(),
            back_status: self.back_status.clone(),
            cached_dense: self.cached_dense.as_ref().map(|dense| dense.dyn_clone()),
        }
    }
}

impl SizeBytes for RangeComponentResultsInner {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            indices,
            promises_front,
            promises_back,
            front_status: _,
            back_status: _,
            cached_dense,
        } = self;

        indices.total_size_bytes()
            + promises_front.total_size_bytes()
            + promises_back.total_size_bytes()
            + cached_dense
                .as_ref()
                .map_or(0, |data| data.dyn_total_size_bytes())
    }
}

impl std::fmt::Debug for RangeComponentResultsInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            indices,
            promises_front: _,
            promises_back: _,
            front_status: _,
            back_status: _,
            cached_dense: _, // we can't, we don't know the type
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

impl RangeComponentResultsInner {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            indices: VecDeque::new(),
            promises_front: Vec::new(),
            promises_back: Vec::new(),
            front_status: (TimeInt::MIN, PromiseResult::Ready(())),
            back_status: (TimeInt::MAX, PromiseResult::Ready(())),
            cached_dense: None,
        }
    }

    /// No-op in release.
    #[inline]
    pub fn sanity_check(&self) {
        if !cfg!(debug_assertions) {
            return;
        }

        let Self {
            indices,
            promises_front,
            promises_back,
            front_status: _,
            back_status: _,
            cached_dense,
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
        if let (Some(p_index), Some(i_index)) = (
            promises_back.first().map(|(index, _)| index),
            indices.back(),
        ) {
            assert!(
                i_index < p_index,
                "the leftmost back promise must have an index larger than the rightmost data index ({i_index:?} < {p_index:?})",
            );
        }

        if let Some(dense) = cached_dense.as_ref() {
            assert_eq!(indices.len(), dense.dyn_num_entries());
        }
    }

    /// Returns the pending time range that will be covered by the cached data.
    ///
    /// Reminder: [`TimeInt::STATIC`] is never included in [`ResolvedTimeRange`]s.
    #[inline]
    pub fn pending_time_range(&self) -> Option<ResolvedTimeRange> {
        let pending_front_min = self.promises_front.first().map(|((t, _), _)| *t);
        let pending_front_max = self.promises_front.last().map(|((t, _), _)| *t);
        let pending_back_max = self.promises_back.last().map(|((t, _), _)| *t);

        let first_time = self.indices.front().map(|(t, _)| *t);
        let last_time = self.indices.back().map(|(t, _)| *t);

        Some(ResolvedTimeRange::new(
            pending_front_min.or(first_time)?,
            pending_back_max.or(last_time).or(pending_front_max)?,
        ))
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

        let time_range = self.pending_time_range();

        let Self {
            indices,
            promises_front,
            promises_back,
            front_status,
            back_status,
            cached_dense,
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
        }

        self.sanity_check();
    }

    #[inline]
    pub fn clear(&mut self) {
        *self = Self::empty();
    }
}
