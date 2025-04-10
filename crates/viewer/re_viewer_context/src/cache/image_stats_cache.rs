use ahash::{HashMap, HashSet};
use itertools::Either;

use re_chunk_store::ChunkStoreEvent;
use re_log_types::hash::Hash64;
use re_types::Component as _;

use crate::{Cache, ImageInfo, ImageStats};

// Caches image stats (use e.g. `RowId` to generate cache key).
#[derive(Default)]
pub struct ImageStatsCache(HashMap<Hash64, HashMap<Hash64, ImageStats>>);

impl ImageStatsCache {
    pub fn entry(&mut self, image: &ImageInfo) -> ImageStats {
        let inner_key = Hash64::hash(image.format);
        *self
            .0
            .entry(image.buffer_cache_key)
            .or_default()
            .entry(inner_key)
            .or_insert_with(|| ImageStats::from_image(image))
    }
}

impl Cache for ImageStatsCache {
    fn purge_memory(&mut self) {
        // Purging the image stats is not worth it - these are very small objects!
    }

    fn on_store_events(&mut self, events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();

        let cache_key_removed: HashSet<Hash64> = events
            .iter()
            .flat_map(|event| {
                let is_deletion = || event.kind == re_chunk_store::ChunkStoreDiffKind::Deletion;
                let contains_image_blob = || {
                    event
                        .chunk
                        .components()
                        .contains_key(&re_types::components::Blob::name())
                };

                if is_deletion() && contains_image_blob() {
                    Either::Left(event.chunk.row_ids().map(Hash64::hash))
                } else {
                    Either::Right(std::iter::empty())
                }
            })
            .collect();

        self.0
            .retain(|cache_key, _per_key| !cache_key_removed.contains(cache_key));
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
