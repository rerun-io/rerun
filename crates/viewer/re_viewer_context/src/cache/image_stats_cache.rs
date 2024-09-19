use egui::util::hash;

use crate::{Cache, ImageInfo, ImageStats};

// Caches image stats using a [`RowId`]
#[derive(Default)]
pub struct ImageStatsCache(ahash::HashMap<u64, ImageStats>);

impl ImageStatsCache {
    pub fn entry(&mut self, image: &ImageInfo) -> ImageStats {
        let key = hash((image.buffer_row_id, image.format));
        *self
            .0
            .entry(key)
            .or_insert_with(|| ImageStats::from_image(image))
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
