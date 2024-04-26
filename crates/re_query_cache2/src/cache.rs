use std::{collections::BTreeSet, sync::Arc};

use ahash::{HashMap, HashSet};
use parking_lot::RwLock;

use re_data_store::{DataStore, StoreDiff, StoreEvent, StoreSubscriber, TimeInt};
use re_log_types::{EntityPath, StoreId, Timeline};
use re_types_core::ComponentName;

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

#[derive(Debug)]
pub struct Caches {
    /// The [`StoreId`] of the associated [`DataStore`].
    pub(crate) store_id: StoreId,

    // NOTE: `Arc` so we can cheaply free the top-level lock early when needed.
    pub(crate) latest_at_per_cache_key: RwLock<HashMap<CacheKey, Arc<RwLock<LatestAtCache>>>>,

    // NOTE: `Arc` so we can cheaply free the top-level lock early when needed.
    pub(crate) range_per_cache_key: RwLock<HashMap<CacheKey, Arc<RwLock<RangeCache>>>>,
}

impl Caches {
    #[inline]
    pub fn new(store: &DataStore) -> Self {
        Self {
            store_id: store.id().clone(),
            latest_at_per_cache_key: Default::default(),
            range_per_cache_key: Default::default(),
        }
    }
}

impl StoreSubscriber for Caches {
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

    fn on_events(&mut self, events: &[StoreEvent]) {
        re_tracing::profile_function!(format!("num_events={}", events.len()));

        #[derive(Default, Debug)]
        struct CompactedEvents {
            static_: HashSet<(EntityPath, ComponentName)>,
            temporal: HashMap<CacheKey, BTreeSet<TimeInt>>,
        }

        let mut compacted = CompactedEvents::default();

        for event in events {
            let StoreEvent {
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

            let StoreDiff {
                kind: _, // Don't care: both additions and deletions invalidate query results.
                row_id: _,
                times,
                entity_path,
                cells,
            } = diff;

            {
                re_tracing::profile_scope!("compact events");

                if times.is_empty() {
                    for component_name in cells.keys() {
                        compacted
                            .static_
                            .insert((entity_path.clone(), *component_name));
                    }
                }

                for &(timeline, data_time) in times {
                    for component_name in cells.keys() {
                        let key = CacheKey::new(entity_path.clone(), timeline, *component_name);
                        let data_times = compacted.temporal.entry(key).or_default();
                        data_times.insert(data_time);
                    }
                }
            }
        }

        let caches_latest_at = self.latest_at_per_cache_key.write();
        let caches_range = self.range_per_cache_key.write();
        // NOTE: Don't release the top-level locks -- even though this cannot happen yet with
        // our current macro-architecture, we want to prevent queries from concurrently
        // running while we're updating the invalidation flags.

        {
            re_tracing::profile_scope!("timeless");

            // TODO(cmc): This is horribly stupid and slow and can easily be made faster by adding
            // yet another layer of caching indirection.
            // But since this pretty much never happens in practice, let's not go there until we
            // have metrics showing that show we need to.
            for (entity_path, component_name) in compacted.static_ {
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
