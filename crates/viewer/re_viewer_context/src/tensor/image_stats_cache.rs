use egui::util::hash;

use crate::{Cache, ImageComponents};

use super::TensorStats;
// Caches image stats using a [`RowId`]
#[derive(Default)]
pub struct ImageStatsCache(ahash::HashMap<u64, TensorStats>);

impl ImageStatsCache {
    pub fn entry(&mut self, image: &ImageComponents) -> TensorStats {
        let key = hash((image.blob_row_id, image.element_type));
        *self
            .0
            .entry(key)
            .or_insert_with(|| TensorStats::from_image(image))
    }
}

impl Cache for ImageStatsCache {
    fn begin_frame(&mut self) {}

    fn purge_memory(&mut self) {
        // Purging the image stats is not worth it - these are very small objects!
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
