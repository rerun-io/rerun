use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ahash::HashMap;
use nohash_hasher::IntSet;
use parking_lot::RwLock;
use re_byte_size::{MemUsageTreeCapture, SizeBytes as _};
use re_chunk::{ChunkId, ComponentIdentifier};
use re_chunk_store::{
    ChunkDirectLineageReport, ChunkStoreDiff, ChunkStoreDiffVirtualAddition, ChunkStoreEvent,
    ChunkStoreHandle, ChunkStoreSubscriber,
};
use re_log_types::{AbsoluteTimeRange, EntityPath, StoreId, TimeInt, TimelineName};
use re_types_core::archetypes;

use crate::{LatestAtCache, RangeCache};

// ---

/// Uniquely identifies cached query results in the [`QueryCache`].
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QueryCacheKey {
    pub entity_path: EntityPath,
    pub timeline_name: TimelineName,
    pub component: ComponentIdentifier,
}

impl re_byte_size::SizeBytes for QueryCacheKey {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            entity_path,
            timeline_name,
            component: component_identifier,
        } = self;
        entity_path.heap_size_bytes()
            + timeline_name.heap_size_bytes()
            + component_identifier.heap_size_bytes()
    }
}

impl std::fmt::Debug for QueryCacheKey {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            entity_path,
            timeline_name,
            component: component_identifier,
        } = self;
        f.write_fmt(format_args!(
            "{entity_path}:{component_identifier} on '{timeline_name}'"
        ))
    }
}

impl QueryCacheKey {
    #[inline]
    pub fn new(
        entity_path: impl Into<EntityPath>,
        timeline: impl Into<TimelineName>,
        component_identifier: ComponentIdentifier,
    ) -> Self {
        Self {
            entity_path: entity_path.into(),
            timeline_name: timeline.into(),
            component: component_identifier,
        }
    }
}

/// A ref-counted, inner-mutable handle to a [`QueryCache`].
///
/// Cheap to clone.
///
/// It is possible to grab the lock behind this handle while _maintaining a static lifetime_, see:
/// * [`QueryCacheHandle::read_arc`]
/// * [`QueryCacheHandle::write_arc`]
#[derive(Clone)]
pub struct QueryCacheHandle(Arc<parking_lot::RwLock<QueryCache>>);

impl QueryCacheHandle {
    #[inline]
    pub fn new(cache: QueryCache) -> Self {
        Self(Arc::new(parking_lot::RwLock::new(cache)))
    }

    #[inline]
    pub fn into_inner(self) -> Arc<parking_lot::RwLock<QueryCache>> {
        self.0
    }
}

impl QueryCacheHandle {
    #[inline]
    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, QueryCache> {
        self.0.read_recursive()
    }

    #[inline]
    pub fn try_read(&self) -> Option<parking_lot::RwLockReadGuard<'_, QueryCache>> {
        self.0.try_read_recursive()
    }

    #[inline]
    pub fn write(&self) -> parking_lot::RwLockWriteGuard<'_, QueryCache> {
        self.0.write()
    }

    #[inline]
    pub fn try_write(&self) -> Option<parking_lot::RwLockWriteGuard<'_, QueryCache>> {
        self.0.try_write()
    }

    #[inline]
    pub fn read_arc(&self) -> parking_lot::ArcRwLockReadGuard<parking_lot::RawRwLock, QueryCache> {
        parking_lot::RwLock::read_arc_recursive(&self.0)
    }

    #[inline]
    pub fn try_read_arc(
        &self,
    ) -> Option<parking_lot::ArcRwLockReadGuard<parking_lot::RawRwLock, QueryCache>> {
        parking_lot::RwLock::try_read_recursive_arc(&self.0)
    }

    #[inline]
    pub fn write_arc(
        &self,
    ) -> parking_lot::ArcRwLockWriteGuard<parking_lot::RawRwLock, QueryCache> {
        parking_lot::RwLock::write_arc(&self.0)
    }

    #[inline]
    pub fn try_write_arc(
        &self,
    ) -> Option<parking_lot::ArcRwLockWriteGuard<parking_lot::RawRwLock, QueryCache>> {
        parking_lot::RwLock::try_write_arc(&self.0)
    }
}

pub struct QueryCache {
    /// Handle to the associated [`ChunkStoreHandle`].
    pub(crate) store: ChunkStoreHandle,

    /// The [`StoreId`] of the associated [`ChunkStoreHandle`].
    pub(crate) store_id: StoreId,

    /// Keeps track of which entities have had any `Clear`-related data on any timeline at any
    /// point in time.
    ///
    /// This is used to optimized read-time clears, so that we don't unnecessarily pay for the fixed
    /// overhead of all the query layers when we know for a fact that there won't be any data there.
    /// This is a huge performance improvement in practice, especially in recordings with many entities.
    pub(crate) might_require_clearing: RwLock<IntSet<EntityPath>>,

    // NOTE: `Arc` so we can cheaply free the top-level lock early when needed.
    pub(crate) latest_at_per_cache_key: RwLock<HashMap<QueryCacheKey, Arc<RwLock<LatestAtCache>>>>,

    // NOTE: `Arc` so we can cheaply free the top-level lock early when needed.
    pub(crate) range_per_cache_key: RwLock<HashMap<QueryCacheKey, Arc<RwLock<RangeCache>>>>,
}

impl re_byte_size::SizeBytes for QueryCache {
    fn heap_size_bytes(&self) -> u64 {
        re_tracing::profile_function!();

        let Self {
            store: _,
            store_id: _,
            might_require_clearing,

            // TODO(RR-3800): better size estimation
            // TODO(RR-3366): this seems to be over-estimating a lot?
            // Maybe double-counting chunks or other arrow data?
            latest_at_per_cache_key: _,

            range_per_cache_key,
        } = self;

        might_require_clearing.heap_size_bytes() + range_per_cache_key.heap_size_bytes()
    }
}

impl MemUsageTreeCapture for QueryCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_tracing::profile_function!();

        let Self {
            store_id: _,
            store: _,
            might_require_clearing,
            latest_at_per_cache_key: _,
            range_per_cache_key,
        } = self;

        re_byte_size::MemUsageNode::new()
            .with_child(
                "might_require_clearing",
                might_require_clearing.total_size_bytes(),
            )
            // TODO(RR-3366): this seems to be over-estimating a lot?
            // Maybe double-counting chunks or other arrow data?
            // .with_child(
            //     "latest_at_per_cache_key",
            //     latest_at_per_cache_key.total_size_bytes(),
            // )
            .with_child(
                "range_per_cache_key",
                range_per_cache_key.total_size_bytes(),
            )
            .into_tree()
    }
}

impl std::fmt::Debug for QueryCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            store_id,
            store,
            might_require_clearing,
            latest_at_per_cache_key,
            range_per_cache_key,
        } = self;

        let mut strings = Vec::new();

        strings.push(format!(
            "[Entities that must be checked for clears @ {store_id:?}]\n"
        ));
        {
            let sorted: BTreeSet<EntityPath> =
                might_require_clearing.read().iter().cloned().collect();
            for entity_path in sorted {
                strings.push(format!("  * {entity_path}\n"));
            }
            strings.push("\n".to_owned());
        }

        strings.push(format!("[LatestAt @ {store_id:?}]"));
        {
            let latest_at_per_cache_key = latest_at_per_cache_key.read();
            let latest_at_per_cache_key: BTreeMap<_, _> = latest_at_per_cache_key.iter().collect();

            for (cache_key, cache) in &latest_at_per_cache_key {
                let cache = cache.read();
                strings.push(format!(
                    "  [{cache_key:?} (pending_invalidation_min={:?})]",
                    cache.pending_invalidations.first().map(|&t| {
                        let range = AbsoluteTimeRange::new(t, TimeInt::MAX);
                        if let Some(time_type) =
                            store.read().time_column_type(&cache_key.timeline_name)
                        {
                            time_type.format_range_utc(range)
                        } else {
                            format!("{range:?}")
                        }
                    })
                ));
                strings.push(indent::indent_all_by(4, format!("{cache:?}")));
            }
        }

        strings.push(format!("[Range @ {store_id:?}]"));
        {
            let range_per_cache_key = range_per_cache_key.read();
            let range_per_cache_key: BTreeMap<_, _> = range_per_cache_key.iter().collect();

            for (cache_key, cache) in &range_per_cache_key {
                let cache = cache.read();
                strings.push(format!(
                    "  [{cache_key:?} (pending_invalidations={:?})]",
                    cache.pending_invalidations,
                ));
                strings.push(indent::indent_all_by(4, format!("{cache:?}")));
            }
        }

        f.write_str(&strings.join("\n").replace("\n\n", "\n"))
    }
}

impl QueryCache {
    #[inline]
    pub fn new(store: ChunkStoreHandle) -> Self {
        let store_id = store.read().id();
        Self {
            store,
            store_id,
            might_require_clearing: Default::default(),
            latest_at_per_cache_key: Default::default(),
            range_per_cache_key: Default::default(),
        }
    }

    #[inline]
    pub fn new_handle(store: ChunkStoreHandle) -> QueryCacheHandle {
        QueryCacheHandle::new(Self::new(store))
    }

    #[inline]
    pub fn clear(&self) {
        let Self {
            store: _,
            store_id: _,
            might_require_clearing,
            latest_at_per_cache_key,
            range_per_cache_key,
        } = self;

        might_require_clearing.write().clear();
        latest_at_per_cache_key.write().clear();
        range_per_cache_key.write().clear();
    }
}

impl ChunkStoreSubscriber for QueryCache {
    #[inline]
    fn name(&self) -> String {
        "rerun.store_subscribers.QueryCache".into()
    }

    #[inline]
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_events(&mut self, events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!(format!("num_events={}", events.len()));

        #[derive(Default, Debug)]
        struct CompactedEvents {
            static_: HashMap<(EntityPath, ComponentIdentifier), BTreeSet<ChunkId>>,
            temporal_latest_at: HashMap<QueryCacheKey, TimeInt>,
            temporal_range: HashMap<QueryCacheKey, BTreeSet<ChunkId>>,
        }

        let mut compacted_events = CompactedEvents::default();

        for event in events {
            let ChunkStoreEvent {
                store_id,
                store_generation: _,
                event_id: _,
                diff,
            } = event;

            assert!(
                self.store_id == *store_id,
                "attempted to use a query cache {:?} with the wrong datastore ({:?})",
                self.store_id,
                store_id,
            );

            match diff {
                ChunkStoreDiff::VirtualAddition(ChunkStoreDiffVirtualAddition { rrd_manifest }) => {
                    re_tracing::profile_scope!("compact event (virtual addition)");

                    // Some virtual data was inserted into the store, we need to keep track of this information.
                    //
                    // In particular, we must know if there are pending tombstones out there, in order to properly
                    // populate the `might_require_clearing` set.

                    for (entity_path, per_component) in rrd_manifest.static_map() {
                        for (component, chunk_id) in per_component {
                            compacted_events
                                .static_
                                .entry((entity_path.clone(), *component))
                                .or_default()
                                .insert(*chunk_id);
                        }
                    }

                    for (entity_path, per_timeline) in rrd_manifest.temporal_map() {
                        for (timeline, per_component) in per_timeline {
                            for (component, per_chunk) in per_component {
                                for (chunk_id, entry) in per_chunk {
                                    let key = QueryCacheKey::new(
                                        entity_path.clone(),
                                        *timeline.name(),
                                        *component,
                                    );

                                    // latest-at
                                    {
                                        let data_time_min = entry.time_range.min();
                                        compacted_events
                                            .temporal_latest_at
                                            .entry(key.clone())
                                            .and_modify(|time| {
                                                *time = TimeInt::min(*time, data_time_min);
                                            })
                                            .or_insert(data_time_min);
                                    }

                                    // range
                                    {
                                        let compacted_events =
                                            compacted_events.temporal_range.entry(key).or_default();
                                        compacted_events.insert(*chunk_id);
                                    }
                                }
                            }
                        }
                    }
                }

                ChunkStoreDiff::Addition(add) => {
                    re_tracing::profile_scope!("compact event (physical addition)");

                    // Some physical data was inserted into the store, we need to invalidate the caches appropriately.

                    // Static
                    if add.chunk_before_processing.is_static() {
                        // For static data, we maintain an actual chunk collection that must maps 1:1 with the current
                        // state of the store, therefore we must work with processed data.
                        let chunk = &add.chunk_after_processing;

                        for component_identifier in chunk.components_identifiers() {
                            let compacted_events = compacted_events
                                .static_
                                .entry((chunk.entity_path().clone(), component_identifier))
                                .or_default();

                            compacted_events.insert(chunk.id());

                            // If a compaction was triggered, make sure to drop the original chunks.
                            //
                            // There's nothing to be done for splits, since the original chunk never made it into
                            // the store anyway.
                            if let ChunkDirectLineageReport::CompactedFrom(chunks) =
                                &add.direct_lineage
                            {
                                compacted_events.extend(chunks.keys().copied());
                            }
                        }
                    }

                    // LatestAt
                    {
                        // For latest-at, we want to make sure to invalidate all chunks beyond the
                        // smallest timestamp that was modified.
                        //
                        // For compaction, that means looking at the complete processed chunk, because even though
                        // this has no semantic impact on the query, we still want to make sure that cache does not
                        // keep strong references to these old chunks that have now been compaced away.
                        let chunk = match &add.direct_lineage {
                            ChunkDirectLineageReport::CompactedFrom(_) => {
                                &add.chunk_after_processing
                            }
                            _ => add.delta_chunk(),
                        };

                        for (timeline, per_component) in chunk.time_range_per_component() {
                            for (component_identifier, time_range) in per_component {
                                let key = QueryCacheKey::new(
                                    chunk.entity_path().clone(),
                                    timeline,
                                    component_identifier,
                                );

                                let data_time_min = time_range.min();
                                compacted_events
                                    .temporal_latest_at
                                    .entry(key.clone())
                                    .and_modify(|time| *time = TimeInt::min(*time, data_time_min))
                                    .or_insert(data_time_min);
                            }
                        }
                    }

                    // Range
                    {
                        // For ranges, we maintain an actual chunk collection that must maps 1:1 with the current
                        // state of the store, therefore we must work with processed data.
                        let chunk = &add.chunk_after_processing;

                        for (timeline, per_component) in chunk.time_range_per_component() {
                            for component_identifier in per_component.keys().copied() {
                                let key = QueryCacheKey::new(
                                    chunk.entity_path().clone(),
                                    timeline,
                                    component_identifier,
                                );

                                let compacted_events =
                                    compacted_events.temporal_range.entry(key).or_default();
                                compacted_events.insert(chunk.id());

                                // If a compaction was triggered, make sure to drop the original chunks.
                                //
                                // There's nothing to be done for splits, since the original chunk never made it into
                                // the store anyway.
                                if let ChunkDirectLineageReport::CompactedFrom(chunks) =
                                    &add.direct_lineage
                                {
                                    compacted_events.extend(chunks.keys().copied());
                                }
                            }
                        }
                    }
                }

                ChunkStoreDiff::Deletion(del) => {
                    re_tracing::profile_scope!("compact event (physical deletion)");

                    // Some physical data was removed from the store, we need to invalidate the caches appropriately.

                    if del.chunk.is_static() {
                        for component_identifier in del.chunk.components_identifiers() {
                            let compacted_events = compacted_events
                                .static_
                                .entry((del.chunk.entity_path().clone(), component_identifier))
                                .or_default();

                            compacted_events.insert(del.chunk.id());
                        }
                    }

                    for (timeline, per_component) in del.chunk.time_range_per_component() {
                        for (component_identifier, time_range) in per_component {
                            let key = QueryCacheKey::new(
                                del.chunk.entity_path().clone(),
                                timeline,
                                component_identifier,
                            );

                            // latest-at
                            {
                                let data_time_min = time_range.min();

                                compacted_events
                                    .temporal_latest_at
                                    .entry(key.clone())
                                    .and_modify(|time| *time = TimeInt::min(*time, data_time_min))
                                    .or_insert(data_time_min);
                            }

                            // range
                            {
                                let compacted_events =
                                    compacted_events.temporal_range.entry(key).or_default();

                                compacted_events.insert(del.chunk.id());
                            }
                        }
                    }
                }
            }
        }

        let mut might_require_clearing = self.might_require_clearing.write();
        let caches_latest_at = self.latest_at_per_cache_key.write();
        let caches_range = self.range_per_cache_key.write();
        // NOTE: Don't release the top-level locks -- even though this cannot happen yet with
        // our current macro-architecture, we want to prevent queries from concurrently
        // running while we're updating the invalidation flags.

        {
            re_tracing::profile_scope!("static");

            // TODO(cmc): This is horribly stupid and slow and can easily be made faster by adding
            // yet another layer of caching indirection.
            // But since this pretty much never happens in practice, let's not go there until we
            // have metrics showing that show we need to.
            for ((entity_path, component_identifier), chunk_ids) in compacted_events.static_ {
                if component_identifier == archetypes::Clear::descriptor_is_recursive().component {
                    might_require_clearing.insert(entity_path.clone());
                }

                for (key, cache) in caches_latest_at.iter() {
                    if key.entity_path == entity_path && key.component == component_identifier {
                        cache.write().pending_invalidations.insert(TimeInt::STATIC);
                    }
                }

                for (key, cache) in caches_range.iter() {
                    if key.entity_path == entity_path && key.component == component_identifier {
                        cache
                            .write()
                            .pending_invalidations
                            .extend(chunk_ids.iter().copied());
                    }
                }
            }
        }

        {
            re_tracing::profile_scope!("temporal");

            for (key, time) in compacted_events.temporal_latest_at {
                if key.component == archetypes::Clear::descriptor_is_recursive().component {
                    might_require_clearing.insert(key.entity_path.clone());
                }

                if let Some(cache) = caches_latest_at.get(&key) {
                    let mut cache = cache.write();
                    cache.pending_invalidations.insert(time);
                }
            }

            for (key, chunk_ids) in compacted_events.temporal_range {
                if let Some(cache) = caches_range.get(&key) {
                    cache
                        .write()
                        .pending_invalidations
                        .extend(chunk_ids.iter().copied());
                }
            }
        }
    }
}
