use parking_lot::{ArcRwLockReadGuard, RawRwLock, RwLock};
use std::sync::Arc;

use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;
use re_tf::TransformCache;
use re_viewer_context::{Cache, CacheMemoryReport};

/// Stores a [`re_tf::TransformCache`] for each recording.
///
/// Ensures that the cache stays up to date.
#[derive(Default)]
pub struct TransformDatabaseCache {
    transform_cache: Arc<RwLock<TransformCache>>,
}

impl TransformDatabaseCache {
    /// Gets read access to the transform cache.
    ///
    /// While the lock is held, no new updates can be applied to the transform cache.
    pub fn read_lock_transform_cache(&self) -> ArcRwLockReadGuard<RawRwLock, TransformCache> {
        self.transform_cache.read_arc()
    }
}

impl Cache for TransformDatabaseCache {
    fn purge_memory(&mut self) {
        // Can't purge memory from the transform cache right now and even if we could, there's
        // no point to it since we can't build it up in a more compact fashion yet.
    }

    fn memory_report(&self) -> CacheMemoryReport {
        CacheMemoryReport {
            // TODO(RR-2516): Implement SizeBytes for TransformCache.
            bytes_cpu: 0, //self.transform_cache.total_size_bytes(),
            bytes_gpu: None,
            per_cache_item_info: Vec::new(),
        }
    }

    fn name(&self) -> &'static str {
        "Transform Database"
    }

    fn on_store_events(&mut self, events: &[&ChunkStoreEvent], entity_db: &EntityDb) {
        re_tracing::profile_function!();

        debug_assert!(
            self.transform_cache.try_write().is_some(),
            "Transform cache is still locked on processing store events. This should never happen."
        );

        self.transform_cache
            .write()
            .apply_all_updates(entity_db, events.iter().copied());
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
