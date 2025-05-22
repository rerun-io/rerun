use ahash::{HashMap, HashSet};
use itertools::Either;

use re_chunk_store::ChunkStoreEvent;
use re_types::{Component as _, components, image::ImageKind};

use crate::{Cache, ImageInfo, ImageStats, image_info::StoredBlobCacheKey};

// Caches image stats (use e.g. `RowId` to generate cache key).
#[derive(Default)]
pub struct ImageStatsCache(HashMap<(StoredBlobCacheKey, ImageKind), ImageStats>);

impl ImageStatsCache {
    pub fn entry(&mut self, image: &ImageInfo) -> ImageStats {
        *self
            .0
            .entry((image.buffer_content_hash, image.kind))
            .or_insert_with(|| ImageStats::from_image(image))
    }
}

impl Cache for ImageStatsCache {
    fn purge_memory(&mut self) {
        // Purging the image stats is not worth it - these are very small objects!
    }

    fn on_store_events(&mut self, events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();

        let cache_key_removed: HashSet<(StoredBlobCacheKey, ImageKind)> = events
            .iter()
            .flat_map(|event| {
                if event.kind == re_chunk_store::ChunkStoreDiffKind::Deletion {
                    Either::Left(
                        event
                            .chunk
                            .component_descriptors()
                            .filter(|descr| descr.component_name == components::Blob::name())
                            .flat_map(|descr| {
                                let kind = ImageKind::from_archetype_name(descr.archetype_name);
                                event.chunk.row_ids().map(move |row_id| {
                                    (StoredBlobCacheKey::new(row_id, &descr), kind)
                                })
                            }),
                    )
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
