use std::{
    cell::OnceCell,
    collections::BTreeMap,
    sync::{Arc, OnceLock},
};

use ahash::{HashMap, HashSet};
use nohash_hasher::IntSet;
use parking_lot::RwLock;

use re_data_store::{DataStore, LatestAtQuery, StoreDiff, StoreEvent, StoreSubscriber, TimeInt};
use re_log_types::{DataCell, EntityPath, RowId, StoreId, Timeline};
use re_types_core::{components::InstanceKey, Archetype, Component, SizeBytes};
use re_types_core::{ComponentName, DeserializationError, DeserializationResult};

use crate::{LatestAtCache, RangeCache};

// --- Data structures ---

/// Uniquely identifies cached query results in the [`Caches`].
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CacheKey {
    /// Which [`EntityPath`] is the query targeting?
    pub entity_path: EntityPath,

    /// Which [`Timeline`] is the query targeting?
    pub timeline: Timeline,

    // TODO
    pub component_name: ComponentName,
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
    /// The [`StoreId`] of the associated [`DataStore`].
    pub(crate) store_id: StoreId,

    // NOTE: `Arc` so we can cheaply free the top-level lock early when needed.
    pub(crate) per_cache_key: RwLock<HashMap<CacheKey, Arc<RwLock<LatestAtCache>>>>,

    // TODO: this makes no sense
    // NOTE: `Arc` so we can cheaply free the top-level lock early when needed.
    pub(crate) per_cache_key_range: RwLock<HashMap<CacheKey, Arc<RwLock<RangeCache>>>>,
}

impl Caches {
    #[inline]
    pub fn new(store: &DataStore) -> Self {
        Self {
            store_id: store.id().clone(),
            per_cache_key: Default::default(),
            per_cache_key_range: Default::default(),
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

            #[derive(Default, Debug)]
            struct CompactedEvents {
                timeless: HashSet<(EntityPath, ComponentName)>,
                timeful: HashMap<CacheKey, IntSet<TimeInt>>,
            }

            // TODO: fuck is all of this done here instead of outside of the loop?

            let mut compacted = CompactedEvents::default();
            {
                re_tracing::profile_scope!("compact events");

                if times.is_empty() {
                    for component_name in cells.keys() {
                        compacted
                            .timeless
                            .insert((entity_path.clone(), *component_name));
                    }
                }

                for &(timeline, data_time) in times {
                    for component_name in cells.keys() {
                        let key = CacheKey::new(entity_path.clone(), timeline, *component_name);
                        let data_times = compacted.timeful.entry(key).or_default();
                        data_times.insert(data_time);
                    }
                }
            }

            // TODO: invalidate ranges

            let caches = self.per_cache_key.write();
            // NOTE: Don't release the top-level lock -- even though this cannot happen yet with
            // our current macro-architecture, we want to prevent queries from concurrently
            // running while we're updating the invalidation flags.

            // TODO(cmc): This is horribly stupid and slow and can easily be made faster by adding
            // yet another layer of caching indirection.
            // But since this pretty much never happens in practice, let's not go there until we
            // have metrics showing that show we need to.
            //
            // TODO: also timeless is mostly going away now
            {
                re_tracing::profile_scope!("timeless");

                for (entity_path, component_name) in compacted.timeless {
                    for (key, cache) in caches.iter() {
                        if key.entity_path == entity_path && key.component_name == component_name {
                            cache.write().pending_timeless_invalidation = true;
                        }
                    }
                }
            }

            {
                re_tracing::profile_scope!("timeful");

                for (key, times) in compacted.timeful {
                    if let Some(cache) = caches.get(&key) {
                        cache
                            .write()
                            .pending_timeful_invalidation
                            .extend(times.clone());
                    }
                }
            }
        }
    }
}
