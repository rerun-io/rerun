use std::sync::Arc;

use ahash::HashMap;

use re_chunk_store::{ChunkStoreEvent, LatestAtQuery};
use re_entity_db::EntityDb;
use re_tf::{TransformForest, TransformResolutionCache};
use re_viewer_context::{Cache, CacheMemoryReport};

/// Stores a [`TransformResolutionCache`] for each recording.
///
/// Ensures that the cache stays up to date.
#[derive(Default)]
pub struct TransformDatabaseStoreCache {
    transform_cache: TransformResolutionCache,

    /// The transform forest may change over time, we store different one for each query we did.
    ///
    /// We currently aggressively purge this every frame, but in the future we may hold on to topology-only changes for longer.
    latest_transform_forest: HashMap<LatestAtQuery, Arc<TransformForest>>,
}

impl TransformDatabaseStoreCache {
    /// Retrieves an existing `TransformForest` for the given query or computes a new one if it doesn't exist.
    pub fn get_or_create_forest(
        &mut self,
        entity_db: &EntityDb,
        query: &LatestAtQuery,
    ) -> Arc<TransformForest> {
        self.latest_transform_forest
            .entry(query.clone())
            .or_insert_with(|| {
                Arc::new(TransformForest::new(
                    entity_db,
                    &self.transform_cache,
                    query,
                ))
            })
            .clone()
    }
}

impl Cache for TransformDatabaseStoreCache {
    fn begin_frame(&mut self) {
        // Discard all transform forests used last frame.
        // TODO(andreas): If our query(/queries) didn't change we could keep them, would be an easy win for static time cursor.
        self.latest_transform_forest.clear();
    }

    fn purge_memory(&mut self) {
        *self = Default::default();
    }

    fn memory_report(&self) -> CacheMemoryReport {
        CacheMemoryReport {
            // TODO(RR-2517): Implement SizeBytes for TransformResolutionCache.
            bytes_cpu: 0, //self.transform_cache.total_size_bytes(),
            bytes_gpu: None,
            per_cache_item_info: Vec::new(),
        }
    }

    fn name(&self) -> &'static str {
        "Transform Database"
    }

    fn on_store_events(&mut self, events: &[&ChunkStoreEvent], _entity_db: &EntityDb) {
        re_tracing::profile_function!();

        self.transform_cache
            .process_store_events(events.iter().copied());
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
