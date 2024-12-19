use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arrow2::array::Array as Arrow2Array;
use nohash_hasher::IntMap;
use parking_lot::RwLock;

use re_byte_size::SizeBytes;
use re_chunk::{Chunk, RowId, UnitChunkShared};
use re_chunk_store::{ChunkStore, LatestAtQuery, TimeInt};
use re_log_types::EntityPath;
use re_types_core::{
    components::ClearIsRecursive, external::arrow::array::ArrayRef, Component, ComponentDescriptor,
    ComponentName,
};

use crate::{QueryCache, QueryCacheKey, QueryError};

// --- Public API ---

/// Compute the ordering of two data indices, making sure to deal with `STATIC` data appropriately.
//
// TODO(cmc): Maybe at some point we'll want to introduce a dedicated `DataIndex` type with
// proper ordering operators etc.
// It's harder than it sounds though -- depending on the context, you don't necessarily want index
// ordering to behave the same way.
fn compare_indices(lhs: (TimeInt, RowId), rhs: (TimeInt, RowId)) -> std::cmp::Ordering {
    match (lhs, rhs) {
        ((TimeInt::STATIC, lhs_row_id), (TimeInt::STATIC, rhs_row_id)) => {
            lhs_row_id.cmp(&rhs_row_id)
        }
        ((_, _), (TimeInt::STATIC, _)) => std::cmp::Ordering::Less,
        ((TimeInt::STATIC, _), (_, _)) => std::cmp::Ordering::Greater,
        _ => lhs.cmp(&rhs),
    }
}

impl QueryCache {
    /// Queries for the given `component_names` using latest-at semantics.
    ///
    /// See [`LatestAtResults`] for more information about how to handle the results.
    ///
    /// This is a cached API -- data will be lazily cached upon access.
    pub fn latest_at<'d>(
        &self,
        query: &LatestAtQuery,
        entity_path: &EntityPath,
        component_descrs: impl IntoIterator<Item = impl Into<Cow<'d, ComponentDescriptor>>>,
    ) -> LatestAtResults {
        // This is called very frequently, don't put a profile scope here.

        let store = self.store.read();

        let mut results = LatestAtResults::empty(entity_path.clone(), query.clone());

        // NOTE: This pre-filtering is extremely important: going through all these query layers
        // has non-negligible overhead even if the final result ends up being nothing, and our
        // number of queries for a frame grows linearly with the number of entity paths.
        let component_names = component_descrs.into_iter().filter_map(|component_descr| {
            let component_descr = component_descr.into();
            store
                .entity_has_component_on_timeline(
                    &query.timeline(),
                    entity_path,
                    &component_descr.component_name,
                )
                .then_some(component_descr.component_name)
        });

        // Query-time clears
        // -----------------
        //
        // We need to find, at query time, whether there exist a `Clear` component that should
        // shadow part or all of the results that we are about to return.
        //
        // This is a two-step process.
        //
        // First, we need to find all `Clear` components that could potentially affect the returned
        // results, i.e. any `Clear` component on the entity itself, or any recursive `Clear`
        // component on any of its recursive parents.
        //
        // Then, we need to compare the index of each component result with the index of the most
        // recent relevant `Clear` component that was found: if there exists a `Clear` component with
        // both a _data time_ lesser or equal to the _query time_ and an index greater or equal
        // than the indexed of the returned data, then we know for sure that the `Clear` shadows
        // the data.
        let mut max_clear_index = (TimeInt::MIN, RowId::ZERO);
        {
            let potential_clears = self.might_require_clearing.read();

            let mut clear_entity_path = entity_path.clone();
            loop {
                if !potential_clears.contains(&clear_entity_path) {
                    // This entity does not contain any `Clear`-related data at all, there's no
                    // point in running actual queries.

                    let Some(parent_entity_path) = clear_entity_path.parent() else {
                        break;
                    };
                    clear_entity_path = parent_entity_path;

                    continue;
                }

                let key = QueryCacheKey::new(
                    clear_entity_path.clone(),
                    query.timeline(),
                    ClearIsRecursive::name(),
                );

                let cache = Arc::clone(
                    self.latest_at_per_cache_key
                        .write()
                        .entry(key.clone())
                        .or_insert_with(|| Arc::new(RwLock::new(LatestAtCache::new(key.clone())))),
                );

                let mut cache = cache.write();
                cache.handle_pending_invalidation();
                if let Some(cached) =
                    cache.latest_at(&store, query, &clear_entity_path, ClearIsRecursive::name())
                {
                    let found_recursive_clear = cached
                        .component_mono::<ClearIsRecursive>()
                        .and_then(Result::ok)
                        == Some(ClearIsRecursive(true.into()));
                    // When checking the entity itself, any kind of `Clear` component
                    // (i.e. recursive or not) will do.
                    //
                    // For (recursive) parents, we need to deserialize the data to make sure the
                    // recursive flag is set.
                    #[allow(clippy::collapsible_if)] // readability
                    if clear_entity_path == *entity_path || found_recursive_clear {
                        if let Some(index) = cached.index(&query.timeline()) {
                            if compare_indices(index, max_clear_index)
                                == std::cmp::Ordering::Greater
                            {
                                max_clear_index = index;
                            }
                        }
                    }
                }

                let Some(parent_entity_path) = clear_entity_path.parent() else {
                    break;
                };

                clear_entity_path = parent_entity_path;
            }
        }

        for component_name in component_names {
            let key = QueryCacheKey::new(entity_path.clone(), query.timeline(), component_name);

            let cache = Arc::clone(
                self.latest_at_per_cache_key
                    .write()
                    .entry(key.clone())
                    .or_insert_with(|| Arc::new(RwLock::new(LatestAtCache::new(key.clone())))),
            );

            let mut cache = cache.write();
            cache.handle_pending_invalidation();
            if let Some(cached) = cache.latest_at(&store, query, entity_path, component_name) {
                // 1. A `Clear` component doesn't shadow its own self.
                // 2. If a `Clear` component was found with an index greater than or equal to the
                //    component data, then we know for sure that it should shadow it.
                if let Some(index) = cached.index(&query.timeline()) {
                    if component_name == ClearIsRecursive::name()
                        || compare_indices(index, max_clear_index) == std::cmp::Ordering::Greater
                    {
                        results.add(component_name, index, cached);
                    }
                }
            }
        }

        results
    }

    /// Free up some RAM by forgetting the older parts of all timelines.
    pub fn purge_fraction_of_ram(&self, fraction_to_purge: f32) {
        re_tracing::profile_function!();

        let mut caches = self.latest_at_per_cache_key.write();
        for (_key, cache) in caches.iter_mut() {
            let mut cache = cache.write();

            let split_point =
                (cache.per_query_time.len().saturating_sub(1) as f32 * fraction_to_purge) as usize;

            if let Some(split_time) = cache.per_query_time.keys().nth(split_point).copied() {
                // NOTE: By not clearing the pending invalidations set, we risk invalidating a
                // future result that need not be invalidated.
                // That is a much better outcome that the opposite though: not invalidating a
                // future result that in fact should have been.
                // See `handle_pending_invalidation` for more information.
                cache.per_query_time = cache.per_query_time.split_off(&split_time);
            }
        }
    }
}

// --- Results ---

/// Results for a latest-at query.
///
/// Use [`LatestAtResults::get`] and/or [`LatestAtResults::get_required`] in order to access
/// the results for each individual component.
#[derive(Debug, Clone)]
pub struct LatestAtResults {
    /// The associated [`EntityPath`].
    pub entity_path: EntityPath,

    /// The query that yielded these results.
    pub query: LatestAtQuery,

    /// The compound index of this query result.
    ///
    /// A latest-at query is a compound operation that gathers data from many different rows.
    /// The index of that compound result corresponds to the index of most the recent row in all the
    /// sub-results, as defined by time and row-id order.
    pub compound_index: (TimeInt, RowId),

    /// Results for each individual component.
    ///
    /// Each [`UnitChunkShared`] MUST always contain the corresponding component.
    pub components: IntMap<ComponentName, UnitChunkShared>,
}

impl LatestAtResults {
    #[inline]
    pub fn empty(entity_path: EntityPath, query: LatestAtQuery) -> Self {
        Self {
            entity_path,
            query,
            compound_index: (TimeInt::STATIC, RowId::ZERO),
            components: Default::default(),
        }
    }
}

impl LatestAtResults {
    #[inline]
    pub fn contains(&self, component_name: &ComponentName) -> bool {
        self.components.contains_key(component_name)
    }

    /// Returns the [`UnitChunkShared`] for the specified [`Component`].
    #[inline]
    pub fn get(&self, component_name: &ComponentName) -> Option<&UnitChunkShared> {
        self.components.get(component_name)
    }

    /// Returns the [`UnitChunkShared`] for the specified [`Component`].
    ///
    /// Returns an error if the component is not present.
    #[inline]
    pub fn get_required(&self, component_name: &ComponentName) -> crate::Result<&UnitChunkShared> {
        if let Some(component) = self.get(component_name) {
            Ok(component)
        } else {
            Err(QueryError::PrimaryNotFound(*component_name))
        }
    }

    /// Returns the compound index (`(TimeInt, RowId)` pair) of the results.
    #[inline]
    pub fn index(&self) -> (TimeInt, RowId) {
        self.compound_index
    }
}

impl LatestAtResults {
    #[doc(hidden)]
    #[inline]
    pub fn add(
        &mut self,
        component_name: ComponentName,
        index: (TimeInt, RowId),
        chunk: UnitChunkShared,
    ) {
        debug_assert!(chunk.num_rows() == 1);

        // NOTE: Since this is a compound API that actually emits multiple queries, the index of the
        // final result is the most recent index among all of its components, as defined by time
        // and row-id order.
        if index > self.compound_index {
            self.compound_index = index;
        }

        self.components.insert(component_name, chunk);
    }
}

// --- Helpers ---
//
// Helpers for UI and other high-level/user-facing code.
//
// In particular, these replace all error handling with logs instead.

impl LatestAtResults {
    // --- Batch ---

    /// Returns the `RowId` for the specified component.
    #[inline]
    pub fn component_row_id(&self, component_name: &ComponentName) -> Option<RowId> {
        self.components
            .get(component_name)
            .and_then(|unit| unit.row_id())
    }

    /// Returns the raw data for the specified component.
    #[inline]
    pub fn component_batch_raw(&self, component_name: &ComponentName) -> Option<ArrayRef> {
        self.components
            .get(component_name)?
            .component_batch_raw(component_name)
    }

    /// Returns the raw data for the specified component.
    #[inline]
    pub fn component_batch_raw_arrow2(
        &self,
        component_name: &ComponentName,
    ) -> Option<Box<dyn Arrow2Array>> {
        self.components
            .get(component_name)
            .and_then(|unit| unit.component_batch_raw_arrow2(component_name))
    }

    /// Returns the deserialized data for the specified component.
    ///
    /// Logs at the specified `log_level` if the data cannot be deserialized.
    #[inline]
    pub fn component_batch_with_log_level<C: Component>(
        &self,
        log_level: re_log::Level,
    ) -> Option<Vec<C>> {
        self.components
            .get(&C::name())
            .and_then(|unit| self.ok_or_log_err(log_level, C::name(), unit.component_batch()?))
    }

    /// Returns the deserialized data for the specified component.
    ///
    /// Logs an error if the data cannot be deserialized.
    #[inline]
    pub fn component_batch<C: Component>(&self) -> Option<Vec<C>> {
        self.component_batch_with_log_level(re_log::Level::Error)
    }

    /// Returns the deserialized data for the specified component.
    #[inline]
    pub fn component_batch_quiet<C: Component>(&self) -> Option<Vec<C>> {
        self.components
            .get(&C::name())
            .and_then(|unit| unit.component_batch()?.ok())
    }

    // --- Instance ---

    /// Returns the raw data for the specified component at the given instance index.
    ///
    /// Logs at the specified `log_level` if the instance index is out of bounds.
    #[inline]
    pub fn component_instance_raw_with_log_level(
        &self,
        log_level: re_log::Level,
        component_name: &ComponentName,
        instance_index: usize,
    ) -> Option<Box<dyn Arrow2Array>> {
        self.components.get(component_name).and_then(|unit| {
            self.ok_or_log_err(
                log_level,
                *component_name,
                unit.component_instance_raw(component_name, instance_index)?,
            )
        })
    }

    /// Returns the raw data for the specified component at the given instance index.
    ///
    /// Logs an error if the instance index is out of bounds.
    #[inline]
    pub fn component_instance_raw(
        &self,
        component_name: &ComponentName,
        instance_index: usize,
    ) -> Option<Box<dyn Arrow2Array>> {
        self.component_instance_raw_with_log_level(
            re_log::Level::Error,
            component_name,
            instance_index,
        )
    }

    /// Returns the raw data for the specified component at the given instance index.
    #[inline]
    pub fn component_instance_raw_quiet(
        &self,
        component_name: &ComponentName,
        instance_index: usize,
    ) -> Option<Box<dyn Arrow2Array>> {
        self.components.get(component_name).and_then(|unit| {
            unit.component_instance_raw(component_name, instance_index)?
                .ok()
        })
    }

    /// Returns the deserialized data for the specified component at the given instance index.
    ///
    /// Logs at the specified `log_level` if the data cannot be deserialized, or if the instance index
    /// is out of bounds.
    #[inline]
    pub fn component_instance_with_log_level<C: Component>(
        &self,
        log_level: re_log::Level,
        instance_index: usize,
    ) -> Option<C> {
        self.components.get(&C::name()).and_then(|unit| {
            self.ok_or_log_err(
                log_level,
                C::name(),
                unit.component_instance(instance_index)?,
            )
        })
    }

    /// Returns the deserialized data for the specified component at the given instance index.
    ///
    /// Logs an error if the data cannot be deserialized, or if the instance index is out of bounds.
    #[inline]
    pub fn component_instance<C: Component>(&self, instance_index: usize) -> Option<C> {
        self.component_instance_with_log_level(re_log::Level::Error, instance_index)
    }

    /// Returns the deserialized data for the specified component at the given instance index.
    ///
    /// Returns an error if the data cannot be deserialized, or if the instance index is out of bounds.
    #[inline]
    pub fn component_instance_quiet<C: Component>(&self, instance_index: usize) -> Option<C> {
        self.components
            .get(&C::name())
            .and_then(|unit| unit.component_instance(instance_index)?.ok())
    }

    // --- Mono ---

    /// Returns the raw data for the specified component, assuming a mono-batch.
    ///
    /// Logs at the specified `log_level` if the underlying batch is not of unit length.
    #[inline]
    pub fn component_mono_raw_with_log_level(
        &self,
        log_level: re_log::Level,
        component_name: &ComponentName,
    ) -> Option<Box<dyn Arrow2Array>> {
        self.components.get(component_name).and_then(|unit| {
            self.ok_or_log_err(
                log_level,
                *component_name,
                unit.component_mono_raw(component_name)?,
            )
        })
    }

    /// Returns the raw data for the specified component, assuming a mono-batch.
    ///
    /// Returns an error if the underlying batch is not of unit length.
    #[inline]
    pub fn component_mono_raw(
        &self,
        component_name: &ComponentName,
    ) -> Option<Box<dyn Arrow2Array>> {
        self.component_mono_raw_with_log_level(re_log::Level::Error, component_name)
    }

    /// Returns the raw data for the specified component, assuming a mono-batch.
    ///
    /// Returns an error if the underlying batch is not of unit length.
    #[inline]
    pub fn component_mono_raw_quiet(
        &self,
        component_name: &ComponentName,
    ) -> Option<Box<dyn Arrow2Array>> {
        self.components
            .get(component_name)
            .and_then(|unit| unit.component_mono_raw(component_name)?.ok())
    }

    /// Returns the deserialized data for the specified component, assuming a mono-batch.
    ///
    /// Logs at the specified `log_level` if the data cannot be deserialized, or if the underlying batch
    /// is not of unit length.
    #[inline]
    pub fn component_mono_with_log_level<C: Component>(
        &self,
        log_level: re_log::Level,
    ) -> Option<C> {
        self.components
            .get(&C::name())
            .and_then(|unit| self.ok_or_log_err(log_level, C::name(), unit.component_mono()?))
    }

    /// Returns the deserialized data for the specified component, assuming a mono-batch.
    ///
    /// Returns an error if the data cannot be deserialized, or if the underlying batch is not of unit length.
    #[inline]
    pub fn component_mono<C: Component>(&self) -> Option<C> {
        self.component_mono_with_log_level(re_log::Level::Error)
    }

    /// Returns the deserialized data for the specified component, assuming a mono-batch.
    ///
    /// Returns an error if the data cannot be deserialized, or if the underlying batch is not of unit length.
    #[inline]
    pub fn component_mono_quiet<C: Component>(&self) -> Option<C> {
        self.components
            .get(&C::name())
            .and_then(|unit| unit.component_mono()?.ok())
    }

    // ---

    fn ok_or_log_err<T>(
        &self,
        log_level: re_log::Level,
        component_name: ComponentName,
        res: re_chunk::ChunkResult<T>,
    ) -> Option<T> {
        match res {
            Ok(data) => Some(data),

            // NOTE: It is expected for UI code to look for OOB instance indices on purpose.
            // E.g. it is very common to look at index 0 in blueprint data that has been cleared.
            Err(re_chunk::ChunkError::IndexOutOfBounds { len: 0, .. }) => None,

            Err(err) => {
                let entity_path = &self.entity_path;
                let index = self.compound_index;
                let err = re_error::format_ref(&err);
                re_log::log_once!(
                    log_level,
                    "Couldn't read {entity_path}:{component_name} @ ({index:?}): {err}",
                );
                None
            }
        }
    }
}

// --- Cached implementation ---

/// Caches the results of `LatestAt` queries for a given [`QueryCacheKey`].
pub struct LatestAtCache {
    /// For debugging purposes.
    pub cache_key: QueryCacheKey,

    /// Organized by _query_ time.
    ///
    /// If the key is present but has a `None` value associated with it, it means we cached the
    /// lack of result.
    /// This is important to do performance-wise: we run _a lot_ of queries each frame to figure
    /// out what to render, and this scales linearly with the number of entity.
    pub per_query_time: BTreeMap<TimeInt, LatestAtCachedChunk>,

    /// These timestamps have been invalidated asynchronously.
    ///
    /// The next time this cache gets queried, it must remove any invalidated entries accordingly.
    ///
    /// Invalidation is deferred to query time because it is far more efficient that way: the frame
    /// time effectively behaves as a natural micro-batching mechanism.
    pub pending_invalidations: BTreeSet<TimeInt>,
}

impl LatestAtCache {
    #[inline]
    pub fn new(cache_key: QueryCacheKey) -> Self {
        Self {
            cache_key,
            per_query_time: Default::default(),
            pending_invalidations: Default::default(),
        }
    }
}

impl std::fmt::Debug for LatestAtCache {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            cache_key,
            per_query_time,
            pending_invalidations: _,
        } = self;

        let mut strings = Vec::new();

        for (query_time, unit) in per_query_time {
            strings.push(format!(
                "query_time={} ({})",
                cache_key.timeline.typ().format_utc(*query_time),
                re_format::format_bytes(unit.total_size_bytes() as _)
            ));
        }

        if strings.is_empty() {
            return f.write_str("<empty>");
        }

        f.write_str(&strings.join("\n").replace("\n\n", "\n"))
    }
}

#[derive(Clone)]
pub struct LatestAtCachedChunk {
    pub unit: UnitChunkShared,

    /// Is this just a reference to another entry in the cache?
    pub is_reference: bool,
}

impl SizeBytes for LatestAtCachedChunk {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            unit: chunk,
            is_reference,
        } = self;

        if *is_reference {
            // This chunk is just a reference to another one in the cache.
            // Consider it amortized.
            0
        } else {
            Chunk::heap_size_bytes(chunk)
        }
    }
}

impl SizeBytes for LatestAtCache {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            cache_key: _,
            per_query_time,
            pending_invalidations,
        } = self;

        let per_query_time = per_query_time.total_size_bytes();
        let pending_invalidations = pending_invalidations.total_size_bytes();

        per_query_time + pending_invalidations
    }
}

impl LatestAtCache {
    /// Queries cached latest-at data for a single component.
    pub fn latest_at(
        &mut self,
        store: &ChunkStore,
        query: &LatestAtQuery,
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) -> Option<UnitChunkShared> {
        // Don't do a profile scope here, this can have a lot of overhead when executing many small queries.
        //re_tracing::profile_scope!("latest_at", format!("{component_name} @ {query:?}"));

        debug_assert_eq!(query.timeline(), self.cache_key.timeline);

        let Self {
            cache_key: _,
            per_query_time,
            pending_invalidations: _,
        } = self;

        if let Some(cached) = per_query_time.get(&query.at()) {
            return Some(cached.unit.clone());
        }

        let ((data_time, _row_id), unit) = store
            .latest_at_relevant_chunks(query, entity_path, component_name)
            .into_iter()
            .filter_map(|chunk| {
                chunk
                    .latest_at(query, component_name)
                    .into_unit()
                    .and_then(|chunk| chunk.index(&query.timeline()).map(|index| (index, chunk)))
            })
            .max_by_key(|(index, _chunk)| *index)?;

        let cached = per_query_time
            .entry(data_time)
            .or_insert_with(|| LatestAtCachedChunk {
                unit,
                is_reference: false,
            })
            .clone();

        // NOTE: Queries that return static data are much cheaper to run, and polluting the query-time cache
        // just to point to the static tables again and again is very wasteful.
        if query.at() != data_time && !data_time.is_static() {
            per_query_time
                .entry(query.at())
                .or_insert_with(|| LatestAtCachedChunk {
                    unit: cached.unit.clone(),
                    is_reference: true,
                });
        }

        Some(cached.unit)
    }

    pub fn handle_pending_invalidation(&mut self) {
        let Self {
            cache_key: _,
            per_query_time,
            pending_invalidations,
        } = self;

        if let Some(oldest_data_time) = pending_invalidations.first() {
            // Remove any data indexed by a _query time_ that's more recent than the oldest
            // _data time_ that's been invalidated.
            //
            // Note that this data time might very well be `TimeInt::STATIC`, in which case the entire
            // query-time-based index will be dropped.
            let discarded = per_query_time.split_off(oldest_data_time);

            // TODO(#5974): Because of non-deterministic ordering, parallelism, and most importantly lack
            // of centralized query layer, it can happen that we try to handle pending invalidations
            // before we even cached the associated data.
            //
            // If that happens, the data will be cached after we've invalidated *nothing*, and will stay
            // there indefinitely since the cache doesn't have a dedicated GC yet.
            //
            // TL;DR: make sure to keep track of pending invalidations indefinitely as long as we
            // haven't had the opportunity to actually invalidate the associated data.
            pending_invalidations.retain(|data_time| {
                let is_reference = discarded
                    .get(data_time)
                    .map_or(true, |chunk| chunk.is_reference);
                !is_reference
            });
        }
    }
}
