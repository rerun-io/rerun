use std::sync::Arc;

use ahash::HashMap;
use re_chunk_store::external::re_chunk::external::re_byte_size::{
    MemUsageTree, MemUsageTreeCapture,
};
use re_viewer_context::{AnnotationMap, Cache, CacheEntryAccess, SizeBytes as _};

struct AnnotationMapCacheAccessor<'a> {
    recording: &'a re_entity_db::EntityDb,
    query: &'a re_chunk_store::LatestAtQuery,
}

/// Caches annotation maps by query for one recording store and one frame.
///
/// [`Self::for_query`] shares each map among all callers through
/// [`re_viewer_context::StoreViewContext::memoizer_read_or_compute`], and frame boundaries discard
/// every entry.
#[derive(Default)]
pub struct AnnotationMapCache {
    maps: HashMap<re_chunk_store::LatestAtQuery, Arc<AnnotationMap>>,
}

impl AnnotationMapCache {
    /// Returns annotations for a recording query, shared by all callers during the frame.
    pub fn for_query(
        ctx: &re_viewer_context::ViewerContext<'_>,
        query: &re_chunk_store::LatestAtQuery,
    ) -> Arc<AnnotationMap> {
        ctx.active_recording_store_view_context()
            .memoizer_read_or_compute::<Self, _, _>(&AnnotationMapCacheAccessor {
                recording: ctx.recording(),
                query,
            })
    }
}

impl CacheEntryAccess<AnnotationMapCacheAccessor<'_>, Arc<AnnotationMap>> for AnnotationMapCache {
    fn read(&self, key: &AnnotationMapCacheAccessor<'_>) -> Option<Arc<AnnotationMap>> {
        self.maps.get(key.query).cloned()
    }

    fn compute(&mut self, key: &AnnotationMapCacheAccessor<'_>) -> Arc<AnnotationMap> {
        self.maps
            .entry(key.query.clone())
            .or_insert_with(|| {
                let mut annotation_map = AnnotationMap::default();
                annotation_map.load(key.recording, key.query);
                Arc::new(annotation_map)
            })
            .clone()
    }
}

impl Cache for AnnotationMapCache {
    fn name(&self) -> &'static str {
        "AnnotationMapCache"
    }

    fn begin_frame(&mut self) {
        // The stored annotation contexts are dependent on the current latest-at query which may change every frame.
        // But they're also fairly cheap to compute, just not so cheap that we want to do it
        // for every visualizer that needs it, so we cache them per frame.
        self.maps.clear();
    }

    fn purge_memory(&mut self) {
        self.maps.clear();
    }
}

impl MemUsageTreeCapture for AnnotationMapCache {
    fn capture_mem_usage_tree(&self) -> MemUsageTree {
        MemUsageTree::Bytes(self.maps.total_size_bytes())
    }
}
