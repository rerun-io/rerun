use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use ahash::{HashMap, HashSet};
use nohash_hasher::IntSet;
use parking_lot::RwLock;

use re_chunk_store::{ChunkStore, ChunkStoreDiff, ChunkStoreEvent, ChunkStoreSubscriber};
use re_log_types::{EntityPath, ResolvedTimeRange, StoreId, TimeInt, Timeline};
use re_types_core::{components::ClearIsRecursive, ComponentName, Loggable as _};

use crate::{LatestAtCache, RangeCache};

// ---

/// Uniquely identifies cached query results in the [`Caches`].
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CacheKey {
    pub entity_path: EntityPath,
    pub timeline: Timeline,
    pub component_name: ComponentName,
}

impl re_types_core::SizeBytes for CacheKey {
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

impl std::fmt::Debug for CacheKey {
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

impl CacheKey {
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

pub struct Caches {
    /// The [`StoreId`] of the associated [`ChunkStore`].
    pub(crate) store_id: StoreId,

    /// Keeps track of which entities have had any `Clear`-related data on any timeline at any
    /// point in time.
    ///
    /// This is used to optimized read-time clears, so that we don't unnecessarily pay for the fixed
    /// overhead of all the query layers when we know for a fact that there won't be any data there.
    /// This is a huge performance improvement in practice, especially in recordings with many entities.
    pub(crate) might_require_clearing: RwLock<IntSet<EntityPath>>,

    // NOTE: `Arc` so we can cheaply free the top-level lock early when needed.
    pub(crate) latest_at_per_cache_key: RwLock<HashMap<CacheKey, Arc<RwLock<LatestAtCache>>>>,

    // NOTE: `Arc` so we can cheaply free the top-level lock early when needed.
    pub(crate) range_per_cache_key: RwLock<HashMap<CacheKey, Arc<RwLock<RangeCache>>>>,
}

impl std::fmt::Debug for Caches {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            store_id,
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
                    "  [{cache_key:?} (pending_invalidation_min={:?})]",
                    cache.pending_invalidation.map(|t| cache_key
                        .timeline
                        .format_time_range_utc(&ResolvedTimeRange::new(t, TimeInt::MAX))),
                ));
                strings.push(indent::indent_all_by(4, format!("{cache:?}")));
            }
        }

        f.write_str(&strings.join("\n").replace("\n\n", "\n"))
    }
}

impl Caches {
    #[inline]
    pub fn new(store: &ChunkStore) -> Self {
        Self {
            store_id: store.id().clone(),
            might_require_clearing: Default::default(),
            latest_at_per_cache_key: Default::default(),
            range_per_cache_key: Default::default(),
        }
    }

    #[inline]
    pub fn clear(&self) {
        let Self {
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

impl ChunkStoreSubscriber for Caches {
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
            static_: HashSet<(EntityPath, ComponentName)>,
            temporal: HashMap<CacheKey, BTreeSet<TimeInt>>,
        }

        let mut compacted = CompactedEvents::default();

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
                compacted: _,
            } = diff;

            {
                re_tracing::profile_scope!("compact events");

                if chunk.is_static() {
                    for component_name in chunk.component_names() {
                        compacted
                            .static_
                            .insert((chunk.entity_path().clone(), component_name));
                    }
                }

                for (&timeline, time_chunk) in chunk.timelines() {
                    for data_time in time_chunk.times() {
                        for component_name in chunk.component_names() {
                            let key = CacheKey::new(
                                chunk.entity_path().clone(),
                                timeline,
                                component_name,
                            );
                            let data_times = compacted.temporal.entry(key).or_default();
                            data_times.insert(data_time);
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
            for (entity_path, component_name) in compacted.static_ {
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
                        cache.write().pending_invalidation = Some(TimeInt::STATIC);
                    }
                }
            }
        }

        {
            re_tracing::profile_scope!("temporal");

            for (key, times) in compacted.temporal {
                if key.component_name == ClearIsRecursive::name() {
                    might_require_clearing.insert(key.entity_path.clone());
                }

                if let Some(cache) = caches_latest_at.get(&key) {
                    cache
                        .write()
                        .pending_invalidations
                        .extend(times.iter().copied());
                }

                if let Some(cache) = caches_range.get(&key) {
                    let pending_invalidation = &mut cache.write().pending_invalidation;
                    let min_time = times.first().copied();
                    *pending_invalidation =
                        Option::min(*pending_invalidation, min_time).or(min_time);
                }
            }
        }
    }
}
