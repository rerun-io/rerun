use std::sync::Arc;

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
    initialized: bool,
    transform_cache: TransformResolutionCache,

    transform_forest: Option<Arc<re_tf::TransformForest>>,
}

impl TransformDatabaseStoreCache {
    fn ensure_initialized(&mut self, entity_db: &EntityDb) {
        if !self.initialized {
            re_tracing::profile_function!();
            self.initialized = true; // There can't be a race here since we have `&mut self``.
            self.transform_cache
                .add_chunks(entity_db.storage_engine().store().iter_physical_chunks());
        }
    }

    /// Returns the registry of all known frame ids.
    #[inline]
    pub fn frame_id_registry(
        &mut self,
        entity_db: &EntityDb,
    ) -> ArcRwLockReadGuard<RawRwLock, FrameIdRegistry> {
        self.ensure_initialized(entity_db);
        self.transform_cache.frame_id_registry()
    }

    /// Accesses the transform component tracking data for a given timeline.
    #[inline]
    pub fn transforms_for_timeline(
        &mut self,
        entity_db: &EntityDb,
        timeline: TimelineName,
    ) -> ArcRwLockReadGuard<RawRwLock, CachedTransformsForTimeline> {
        // TODO(RR-3539): lazily initialize only the requested timeline
        self.ensure_initialized(entity_db);
        self.transform_cache.transforms_for_timeline(timeline)
    }

    pub fn update_transform_forest(&mut self, entity_db: &EntityDb, query: &LatestAtQuery) {
        self.ensure_initialized(entity_db);
        self.transform_forest = Some(Arc::new(TransformForest::new(
            entity_db,
            &self.transform_cache,
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
            initialized,
            transform_cache,
            transform_forest,
        } = self;

        initialized.heap_size_bytes()
            + transform_cache.heap_size_bytes()
            + transform_forest.heap_size_bytes()
    }
}

impl Cache for TransformDatabaseStoreCache {
    fn name(&self) -> &'static str {
        "TransformDatabaseStoreCache"
    }

    fn purge_memory(&mut self) {
        // Can't purge memory from the transform cache right now and even if we could, there's
        // no point to it since we can't build it up in a more compact fashion yet.
    }

    fn on_store_events(&mut self, events: &[&ChunkStoreEvent], _entity_db: &EntityDb) {
        re_tracing::profile_function!();

        self.transform_cache
            .process_store_events(events.iter().copied());
    }
}

impl re_byte_size::MemUsageTreeCapture for TransformDatabaseStoreCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_tracing::profile_function!();

        let Self {
            initialized: _, // just a bool
            transform_cache,
            transform_forest,
        } = self;

        let mut node = re_byte_size::MemUsageNode::new();

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
