use std::sync::Arc;

use ahash::HashSet;
use parking_lot::{ArcRwLockReadGuard, RawRwLock};
use re_byte_size::SizeBytes;
use re_chunk::{LatestAtQuery, TimelineName};
use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;
use re_tf::{
    CachedTransformsForTimeline, FrameIdRegistry, TransformForest, TransformResolutionCache,
};

use super::Cache;

/// Stores a [`TransformResolutionCache`] for each recording.
///
/// Ensures that the cache stays up to date.
#[derive(Default)]
pub struct TransformDatabaseStoreCache {
    transform_cache: Option<TransformResolutionCache>,

    transform_forest: Option<Arc<re_tf::TransformForest>>,

    /// Timelines that were used in the current frame.
    /// Used for evicting unused timelines at the beginning of the next frame.
    used_timelines: HashSet<TimelineName>,
}

impl TransformDatabaseStoreCache {
    /// Returns the registry of all known frame ids.
    #[inline]
    pub fn frame_id_registry(
        &mut self,
        entity_db: &EntityDb,
    ) -> ArcRwLockReadGuard<RawRwLock, FrameIdRegistry> {
        let transform_cache = self
            .transform_cache
            .get_or_insert_with(|| TransformResolutionCache::new(entity_db));

        transform_cache.frame_id_registry()
    }

    /// Accesses the transform component tracking data for a given timeline.
    #[inline]
    pub fn transforms_for_timeline(
        &mut self,
        entity_db: &EntityDb,
        timeline: TimelineName,
    ) -> ArcRwLockReadGuard<RawRwLock, CachedTransformsForTimeline> {
        let transform_cache = self
            .transform_cache
            .get_or_insert_with(|| TransformResolutionCache::new(entity_db));

        // Remember that this timeline was used this frame.
        self.used_timelines.insert(timeline);

        transform_cache
            .ensure_timeline_is_initialized(entity_db.storage_engine().store(), timeline);

        transform_cache.transforms_for_timeline(timeline)
    }

    pub fn update_transform_forest(&mut self, entity_db: &EntityDb, query: &LatestAtQuery) {
        let transform_cache = self
            .transform_cache
            .get_or_insert_with(|| TransformResolutionCache::new(entity_db));

        self.transform_forest = Some(Arc::new(TransformForest::new(
            entity_db,
            transform_cache,
            query,
        )));
    }

    pub fn transform_forest(&self) -> Option<Arc<re_tf::TransformForest>> {
        self.transform_forest.clone()
    }
}

impl SizeBytes for TransformDatabaseStoreCache {
    fn heap_size_bytes(&self) -> u64 {
        re_tracing::profile_function!();

        let Self {
            transform_cache,
            transform_forest,
            used_timelines,
        } = self;

        transform_cache.heap_size_bytes()
            + transform_forest.heap_size_bytes()
            + used_timelines.heap_size_bytes()
    }
}

impl Cache for TransformDatabaseStoreCache {
    fn name(&self) -> &'static str {
        "TransformDatabaseStoreCache"
    }

    fn begin_frame(&mut self) {
        self.used_timelines.clear();
    }

    fn purge_memory(&mut self) {
        if let Some(transform_cache) = &mut self.transform_cache {
            // Evict all timelines that weren't used in the last frame.
            // They will be lazily re-initialized if needed again.
            let unused_timelines = transform_cache
                .cached_timelines()
                .filter(|t| !self.used_timelines.contains(t))
                .collect::<Vec<_>>();

            for timeline in unused_timelines {
                transform_cache.evict_timeline_cache(timeline);
            }
        }
    }

    fn on_store_events(&mut self, events: &[&ChunkStoreEvent], _entity_db: &EntityDb) {
        re_tracing::profile_function!();

        if let Some(transform_cache) = &mut self.transform_cache {
            transform_cache.process_store_events(events.iter().copied());
        }
    }
}

impl re_byte_size::MemUsageTreeCapture for TransformDatabaseStoreCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_tracing::profile_function!();

        let Self {
            used_timelines,
            transform_cache,
            transform_forest,
        } = self;

        let mut node = re_byte_size::MemUsageNode::new();

        node.add("used_timelines", used_timelines.total_size_bytes());
        node.add("transform_cache", transform_cache.capture_mem_usage_tree());

        if let Some(transform_forest) = &transform_forest {
            node.add(
                "transform_forest",
                transform_forest.capture_mem_usage_tree(),
            );
        }

        node.into_tree()
    }
}
