use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use ahash::HashMap;
use nohash_hasher::IntSet;
use parking_lot::RwLock;

use re_chunk::ChunkId;
use re_chunk_store::{
    ChunkCompactionReport, ChunkStoreDiff, ChunkStoreEvent, ChunkStoreHandle, ChunkStoreSubscriber,
};
use re_log_types::{EntityPath, ResolvedTimeRange, StoreId, TimeInt, Timeline};
use re_types_core::{components::ClearIsRecursive, Component as _, ComponentName};

use crate::{LatestAtCache, RangeCache};

// ---

/// Uniquely identifies cached query results in the [`QueryCache`].
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QueryCacheKey {
    pub entity_path: EntityPath,
    pub timeline: Timeline,
    pub component_name: ComponentName,
}

impl re_byte_size::SizeBytes for QueryCacheKey {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            entity_path,
            timeline,
            component_name,
        } = self;
        entity_path.heap_size_bytes()
            + timeline.heap_size_bytes()
            + component_name.heap_size_bytes()
    }
}

impl std::fmt::Debug for QueryCacheKey {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            entity_path,
            timeline,
            component_name,
        } = self;
        f.write_fmt(format_args!(
            "{entity_path}:{component_name} on {}",
            timeline.name()
        ))
    }
}

impl QueryCacheKey {
    #[inline]
    pub fn new(
        entity_path: impl Into<EntityPath>,
        timeline: impl Into<Timeline>,
        component_name: impl Into<ComponentName>,
    ) -> Self {
        Self {
            entity_path: entity_path.into(),
            timeline: timeline.into(),
            component_name: component_name.into(),
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

impl std::fmt::Debug for QueryCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            store_id,
            store: _,
            might_require_clearing,
            latest_at_per_cache_key,
            range_per_cache_key,
        } = self;

        let mut strings = Vec::new();

        strings.push(format!(
            "[Entities that must be checked for clears @ {store_id}]\n"
        ));
        {
            let sorted: BTreeSet<EntityPath> =
                might_require_clearing.read().iter().cloned().collect();
            for entity_path in sorted {
                strings.push(format!("  * {entity_path}\n"));
            }
            strings.push("\n".to_owned());
        }

        strings.push(format!("[LatestAt @ {store_id}]"));
        {
            let latest_at_per_cache_key = latest_at_per_cache_key.read();
            let latest_at_per_cache_key: BTreeMap<_, _> = latest_at_per_cache_key.iter().collect();

            for (cache_key, cache) in &latest_at_per_cache_key {
                let cache = cache.read();
                strings.push(format!(
                    "  [{cache_key:?} (pending_invalidation_min={:?})]",
                    cache.pending_invalidations.first().map(|&t| cache_key
                        .timeline
                        .format_time_range_utc(&ResolvedTimeRange::new(t, TimeInt::MAX))),
                ));
                strings.push(indent::indent_all_by(4, format!("{cache:?}")));
            }
        }

        strings.push(format!("[Range @ {store_id}]"));
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
            static_: HashMap<(EntityPath, ComponentName), BTreeSet<ChunkId>>,
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
                "attempted to use a query cache {} with the wrong datastore ({})",
                self.store_id,
                store_id,
            );

            let ChunkStoreDiff {
                kind: _, // Don't care: both additions and deletions invalidate query results.
                chunk,
                compacted,
            } = diff;

            {
                re_tracing::profile_scope!("compact events");

                if chunk.is_static() {
                    for component_name in chunk.component_names() {
                        let compacted_events = compacted_events
                            .static_
                            .entry((chunk.entity_path().clone(), component_name))
                            .or_default();

                        compacted_events.insert(chunk.id());
                        // If a compaction was triggered, make sure to drop the original chunks too.
                        compacted_events.extend(compacted.iter().flat_map(
                            |ChunkCompactionReport {
                                 srcs: compacted_chunks,
                                 new_chunk: _,
                             }| compacted_chunks.keys().copied(),
                        ));
                    }
                }

                for (timeline, per_component) in chunk.time_range_per_component() {
                    for (component_name, per_desc) in per_component {
                        for (component_desc, time_range) in per_desc {
                            let key = QueryCacheKey::new(
                                chunk.entity_path().clone(),
                                timeline,
                                component_name,
                            );

                            // latest-at
                            {
                                let mut data_time_min = time_range.min();

                                // If a compaction was triggered, make sure to drop the original chunks too.
                                if let Some(ChunkCompactionReport {
                                    srcs: compacted_chunks,
                                    new_chunk: _,
                                }) = compacted
                                {
                                    for chunk in compacted_chunks.values() {
                                        let data_time_compacted = chunk
                                            .time_range_per_component()
                                            .get(&timeline)
                                            .and_then(|per_component| {
                                                per_component.get(&component_name).and_then(
                                                    |per_desc| per_desc.get(&component_desc),
                                                )
                                            })
                                            .map_or(TimeInt::MAX, |time_range| time_range.min());

                                        data_time_min =
                                            TimeInt::min(data_time_min, data_time_compacted);
                                    }
                                }

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

                                compacted_events.insert(chunk.id());
                                // If a compaction was triggered, make sure to drop the original chunks too.
                                compacted_events.extend(compacted.iter().flat_map(
                                    |ChunkCompactionReport {
                                         srcs: compacted_chunks,
                                         new_chunk: _,
                                     }| {
                                        compacted_chunks.keys().copied()
                                    },
                                ));
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
            for ((entity_path, component_name), chunk_ids) in compacted_events.static_ {
                if component_name == ClearIsRecursive::name() {
                    might_require_clearing.insert(entity_path.clone());
                }

                for (key, cache) in caches_latest_at.iter() {
                    if key.entity_path == entity_path && key.component_name == component_name {
                        cache.write().pending_invalidations.insert(TimeInt::STATIC);
                    }
                }

                for (key, cache) in caches_range.iter() {
                    if key.entity_path == entity_path && key.component_name == component_name {
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
                if key.component_name == ClearIsRecursive::name() {
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
