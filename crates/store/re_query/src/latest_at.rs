use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow::array::ArrayRef as ArrowArrayRef;
use nohash_hasher::IntMap;
use parking_lot::RwLock;
use re_byte_size::SizeBytes;
use re_chunk::{Chunk, ChunkId, ComponentIdentifier, RowId, UnitChunkShared};
use re_chunk_store::{ChunkStore, LatestAtQuery, TimeInt};
use re_log_types::EntityPath;
use re_types_core::components::ClearIsRecursive;
use re_types_core::external::arrow::array::ArrayRef;
use re_types_core::{Component, archetypes};

use crate::{QueryCache, QueryCacheKey, QueryError};

// TODO: test partial stuff
// -> and i guess we're gonna have to test clears in particular 😒

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
    /// Queries for the given [`ComponentIdentifier`]s using latest-at semantics.
    ///
    /// See [`LatestAtResults`] for more information about how to handle the results.
    ///
    /// This is a cached API -- data will be lazily cached upon access.
    pub fn latest_at(
        &self,
        query: &LatestAtQuery,
        entity_path: &EntityPath,
        components: impl IntoIterator<Item = ComponentIdentifier>,
    ) -> LatestAtResults {
        // This is called very frequently, don't put a profile scope here.

        let store = self.store.read();

        let mut results = LatestAtResults::empty(entity_path.clone(), query.clone());

        // NOTE: This pre-filtering is extremely important: going through all these query layers
        // has non-negligible overhead even if the final result ends up being nothing, and our
        // number of queries for a frame grows linearly with the number of entity paths.
        let components = components.into_iter().filter(|component| {
            store.entity_has_component_on_timeline(&query.timeline(), entity_path, *component)
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
        //
        // TODO: I guess that, since we have to deal with clear semantics on the latest-at path,
        // we just cannot cache nor return any result at all in case of partial data.
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

                let component = archetypes::Clear::descriptor_is_recursive().component;
                let key =
                    QueryCacheKey::new(clear_entity_path.clone(), query.timeline(), component);

                let cache = Arc::clone(
                    self.latest_at_per_cache_key
                        .write()
                        .entry(key.clone())
                        .or_insert_with(|| Arc::new(RwLock::new(LatestAtCache::new(key)))),
                );

                let mut cache = cache.write();
                cache.handle_pending_invalidation();

                let (cached, missing) =
                    cache.latest_at(&store, query, &clear_entity_path, component);
                if cfg!(debug_assertions) && !missing.is_empty() {
                    debug_assert!(
                        cached.is_none(),
                        "should never have partial latest-at results"
                    );
                }
                results.missing.extend(missing);

                if let Some(cached) = cached {
                    // TODO(andreas): Should clear also work if the component is not fully tagged?
                    let found_recursive_clear = cached
                        .component_mono::<ClearIsRecursive>(component)
                        .and_then(Result::ok)
                        == Some(ClearIsRecursive(true.into()));
                    // When checking the entity itself, any kind of `Clear` component
                    // (i.e. recursive or not) will do.
                    //
                    // For (recursive) parents, we need to deserialize the data to make sure the
                    // recursive flag is set.
                    if (clear_entity_path == *entity_path || found_recursive_clear)
                        && let Some(index) = cached.index(&query.timeline())
                        && compare_indices(index, max_clear_index) == std::cmp::Ordering::Greater
                    {
                        max_clear_index = index;
                    }
                }

                let Some(parent_entity_path) = clear_entity_path.parent() else {
                    break;
                };

                clear_entity_path = parent_entity_path;
            }
        }

        for component in components {
            let key = QueryCacheKey::new(entity_path.clone(), query.timeline(), component);

            let cache = Arc::clone(
                self.latest_at_per_cache_key
                    .write()
                    .entry(key.clone())
                    .or_insert_with(|| Arc::new(RwLock::new(LatestAtCache::new(key)))),
            );

            let mut cache = cache.write();
            cache.handle_pending_invalidation();

            let (cached, missing) = cache.latest_at(&store, query, entity_path, component);
            if cfg!(debug_assertions) && !missing.is_empty() {
                debug_assert!(
                    cached.is_none(),
                    "should never have partial latest-at results"
                );
            }
            results.missing.extend(missing);

            if let Some(cached) = cached {
                // 1. A `Clear` component doesn't shadow its own self.
                // 2. If a `Clear` component was found with an index greater than or equal to the
                //    component data, then we know for sure that it should shadow it.
                if let Some(index) = cached.index(&query.timeline())
                    && (component == archetypes::Clear::descriptor_is_recursive().component
                        || compare_indices(index, max_clear_index) == std::cmp::Ordering::Greater)
                {
                    results.add(component, index, cached);
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

    // TODO: i guess we gotta redo all the same logic right?
    pub missing: Vec<ChunkId>,

    /// The compound index of this query result.
    ///
    /// A latest-at query is a compound operation that gathers data from many different rows.
    /// The index of that compound result corresponds to the index of most the recent row in all the
    /// sub-results, as defined by time and row-id order.
    pub compound_index: (TimeInt, RowId),

    /// Results for each individual component.
    ///
    /// Each [`UnitChunkShared`] MUST always contain the corresponding component.
    pub components: IntMap<ComponentIdentifier, UnitChunkShared>,
}

impl LatestAtResults {
    #[inline]
    pub fn empty(entity_path: EntityPath, query: LatestAtQuery) -> Self {
        Self {
            entity_path,
            query,
            missing: Default::default(),
            compound_index: (TimeInt::STATIC, RowId::ZERO),
            components: Default::default(),
        }
    }
}

impl LatestAtResults {
    /// Returns the [`UnitChunkShared`] for the specified [`Component`].
    pub fn get(&self, component: ComponentIdentifier) -> Option<&UnitChunkShared> {
        self.components.get(&component)
    }

    /// Returns the [`UnitChunkShared`] for the specified [`Component`].
    ///
    /// Returns an error if the component is not present.
    #[inline]
    pub fn get_required(&self, component: ComponentIdentifier) -> crate::Result<&UnitChunkShared> {
        if let Some(component) = self.components.get(&component) {
            Ok(component)
        } else {
            Err(QueryError::PrimaryNotFound(component))
        }
    }

    /// Returns the compound index (`(TimeInt, RowId)` pair) of the results.
    #[inline]
    pub fn index(&self) -> (TimeInt, RowId) {
        self.compound_index
    }
}

impl LatestAtResults {
    #[doc(hidden)] // used by the visualizer overrides sub-system
    #[inline]
    pub fn add(
        &mut self,
        component: ComponentIdentifier,
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

        self.components.insert(component, chunk);
    }
}

// --- Helpers ---
//
// Helpers for UI and other high-level/user-facing code.
//
// In particular, these replace all error handling with logs instead.

// TODO: yeah these are gonna make things awkward im sure... but then again we can address that at
// a later time, since it all falls in the realm of caller responsibility anyhow... or does it

impl LatestAtResults {
    // --- Batch ---

    /// Returns the `RowId` for the specified component.
    #[inline]
    pub fn component_row_id(&self, component: ComponentIdentifier) -> Option<RowId> {
        self.components.get(&component)?.row_id()
    }

    /// Returns the raw data for the specified component.
    #[inline]
    pub fn component_batch_raw(&self, component: ComponentIdentifier) -> Option<ArrayRef> {
        self.components
            .get(&component)?
            .component_batch_raw(component)
    }

    /// Returns the deserialized data for the specified component.
    ///
    /// Logs at the specified `log_level` if the data cannot be deserialized.
    #[inline]
    pub fn component_batch_with_log_level<C: Component>(
        &self,
        log_level: re_log::Level,
        component: ComponentIdentifier,
    ) -> Option<Vec<C>> {
        let unit = self.components.get(&component)?;
        self.ok_or_log_err(log_level, component, unit.component_batch(component)?)
    }

    /// Returns the deserialized data for the specified component.
    ///
    /// Logs an error if the data cannot be deserialized.
    #[inline]
    pub fn component_batch<C: Component>(&self, component: ComponentIdentifier) -> Option<Vec<C>> {
        self.component_batch_with_log_level(re_log::Level::Error, component)
    }

    /// Returns the deserialized data for the specified component.
    #[inline]
    pub fn component_batch_quiet<C: Component>(
        &self,
        component: ComponentIdentifier,
    ) -> Option<Vec<C>> {
        let unit = self.components.get(&component)?;
        unit.component_batch(component)?.ok()
    }

    // --- Instance ---

    /// Returns the raw data for the specified component at the given instance index.
    ///
    /// Logs at the specified `log_level` if the instance index is out of bounds.
    #[inline]
    pub fn component_instance_raw_with_log_level(
        &self,
        log_level: re_log::Level,
        component: ComponentIdentifier,
        instance_index: usize,
    ) -> Option<ArrowArrayRef> {
        let unit = self.components.get(&component)?;
        self.ok_or_log_err(
            log_level,
            component,
            unit.component_instance_raw(component, instance_index)?,
        )
    }

    /// Returns the raw data for the specified component at the given instance index.
    ///
    /// Logs an error if the instance index is out of bounds.
    #[inline]
    pub fn component_instance_raw(
        &self,
        component: ComponentIdentifier,
        instance_index: usize,
    ) -> Option<ArrowArrayRef> {
        self.component_instance_raw_with_log_level(re_log::Level::Error, component, instance_index)
    }

    /// Returns the raw data for the specified component at the given instance index.
    #[inline]
    pub fn component_instance_raw_quiet(
        &self,
        component: ComponentIdentifier,
        instance_index: usize,
    ) -> Option<ArrowArrayRef> {
        let unit = self.components.get(&component)?;
        unit.component_instance_raw(component, instance_index)?.ok()
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
        component: ComponentIdentifier,
    ) -> Option<C> {
        let unit = self.components.get(&component)?;
        self.ok_or_log_err(
            log_level,
            component,
            unit.component_instance(component, instance_index)?,
        )
    }

    /// Returns the deserialized data for the specified component at the given instance index.
    ///
    /// Logs an error if the data cannot be deserialized, or if the instance index is out of bounds.
    #[inline]
    pub fn component_instance<C: Component>(
        &self,
        instance_index: usize,
        component: ComponentIdentifier,
    ) -> Option<C> {
        self.component_instance_with_log_level(re_log::Level::Error, instance_index, component)
    }

    /// Returns the deserialized data for the specified component at the given instance index.
    ///
    /// Returns an error if the data cannot be deserialized, or if the instance index is out of bounds.
    #[inline]
    pub fn component_instance_quiet<C: Component>(
        &self,
        component: ComponentIdentifier,
        instance_index: usize,
    ) -> Option<C> {
        let unit = self.components.get(&component)?;
        unit.component_instance(component, instance_index)?.ok()
    }

    // --- Mono ---

    /// Returns the raw data for the specified component, assuming a mono-batch.
    ///
    /// Logs at the specified `log_level` if the underlying batch is not of unit length.
    #[inline]
    pub fn component_mono_raw_with_log_level(
        &self,
        log_level: re_log::Level,
        component: ComponentIdentifier,
    ) -> Option<ArrowArrayRef> {
        let unit = self.components.get(&component)?;
        self.ok_or_log_err(log_level, component, unit.component_mono_raw(component)?)
    }

    /// Returns the raw data for the specified component, assuming a mono-batch.
    ///
    /// Returns an error if the underlying batch is not of unit length.
    #[inline]
    pub fn component_mono_raw(&self, component: ComponentIdentifier) -> Option<ArrowArrayRef> {
        self.component_mono_raw_with_log_level(re_log::Level::Error, component)
    }

    /// Returns the raw data for the specified component, assuming a mono-batch.
    ///
    /// Returns an error if the underlying batch is not of unit length.
    #[inline]
    pub fn component_mono_raw_quiet(
        &self,
        component: ComponentIdentifier,
    ) -> Option<ArrowArrayRef> {
        let unit = self.components.get(&component)?;
        unit.component_mono_raw(component)?.ok()
    }

    /// Returns the deserialized data for the specified component, assuming a mono-batch.
    ///
    /// Logs at the specified `log_level` if the data cannot be deserialized, or if the underlying batch
    /// is not of unit length.
    #[inline]
    pub fn component_mono_with_log_level<C: Component>(
        &self,
        component: ComponentIdentifier,
        log_level: re_log::Level,
    ) -> Option<C> {
        let unit = self.components.get(&component)?;
        self.ok_or_log_err(log_level, component, unit.component_mono(component)?)
    }

    /// Returns the deserialized data for the specified component, assuming a mono-batch.
    ///
    /// Logs an error if the data cannot be deserialized, or if the underlying batch is not of unit length.
    #[inline]
    pub fn component_mono<C: Component>(&self, component: ComponentIdentifier) -> Option<C> {
        self.component_mono_with_log_level(component, re_log::Level::Error)
    }

    /// Returns the deserialized data for the specified component, assuming a mono-batch.
    ///
    /// Returns none if the data cannot be deserialized, or if the underlying batch is not of unit length.
    #[inline]
    pub fn component_mono_quiet<C: Component>(&self, component: ComponentIdentifier) -> Option<C> {
        let unit = self.components.get(&component)?;
        unit.component_mono(component)?.ok()
    }

    // ---

    fn ok_or_log_err<T>(
        &self,
        log_level: re_log::Level,
        component: ComponentIdentifier,
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
                    "Couldn't read {entity_path}:{component} @ ({index:?}): {err}",
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
            cache_key: _,
            per_query_time,
            pending_invalidations: _,
        } = self;

        let mut strings = Vec::new();

        for (query_time, unit) in per_query_time {
            strings.push(format!(
                "query_time={query_time:?} ({})",
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
    ///
    /// Returns `(cached_unit_chunk, missing_chunk_ids)`.
    fn latest_at(
        &mut self,
        store: &ChunkStore,
        query: &LatestAtQuery,
        entity_path: &EntityPath,
        component: ComponentIdentifier,
    ) -> (Option<UnitChunkShared>, Vec<ChunkId>) {
        // Don't do a profile scope here, this can have a lot of overhead when executing many small queries.
        //re_tracing::profile_scope!("latest_at", format!("{component_type} @ {query:?}"));

        debug_assert_eq!(query.timeline(), self.cache_key.timeline_name);

        let Self {
            cache_key: _,
            per_query_time,
            pending_invalidations: _,
        } = self;

        if let Some(cached) = per_query_time.get(&query.at()) {
            return (Some(cached.unit.clone()), vec![]);
        }

        let results = store.latest_at_relevant_chunks(query, entity_path, component);
        if results.is_partial() {
            // Contrary to range results, partial latest-at results cannot ever be correct on their own,
            // therefore we must give up the current query entirely.
            return (None, results.missing);
        }
        let Some(((data_time, _row_id), unit)) = results
            .chunks
            .into_iter()
            .filter_map(|chunk| {
                let chunk = chunk.latest_at(query, component).into_unit()?;
                chunk.index(&query.timeline()).map(|index| (index, chunk))
            })
            .max_by_key(|(index, _chunk)| *index)
        else {
            return (None, vec![]);
        };

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

        (Some(cached.unit), vec![])
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
                    .is_none_or(|chunk| chunk.is_reference);
                !is_reference
            });
        }
    }
}
