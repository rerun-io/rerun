use re_chunk::RowId;

use super::TensorStats;
use crate::{Cache, ImageComponents};

/// Caches image stats using a [`RowId`]
#[derive(Default)]
pub struct ImageStatsCache(ahash::HashMap<RowId, TensorStats>);

impl ImageStatsCache {
    /// The key should be the `RowId` of the blob
    pub fn entry(&mut self, image: &ImageComponents) -> TensorStats {
        *self
            .0
            .entry(image.row_id)
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
