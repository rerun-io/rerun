use ahash::{HashMap, HashSet};
use itertools::Either;

use re_chunk::RowId;
use re_chunk_store::ChunkStoreEvent;
use re_log_types::hash::Hash64;
use re_types::Component as _;

use crate::{Cache, ImageInfo, ImageStats};

// Caches image stats using a [`RowId`]
#[derive(Default)]
pub struct ImageStatsCache(HashMap<RowId, HashMap<Hash64, ImageStats>>);

impl ImageStatsCache {
    pub fn entry(&mut self, image: &ImageInfo) -> ImageStats {
        let inner_key = Hash64::hash(image.format);
        *self
            .0
            .entry(image.buffer_row_id)
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

        let row_ids_removed: HashSet<RowId> = events
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
                    Either::Left(event.chunk.row_ids())
                } else {
                    Either::Right(std::iter::empty())
                }
            })
            .collect();

        self.0
            .retain(|row_id, _per_key| !row_ids_removed.contains(row_id));
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
