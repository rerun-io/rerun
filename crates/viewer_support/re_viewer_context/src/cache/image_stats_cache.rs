use ahash::HashMap;
use re_byte_size::SizeBytes as _;
use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;
use re_sdk_types::image::ImageKind;

use crate::cache::filter_blob_removed_events;
use crate::image_info::StoredBlobCacheKey;
use crate::{AppCache, Cache, CacheEntryAccess, ImageInfo, ImageStats};

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

impl CacheEntryAccess<ImageInfo, ImageStats> for ImageStatsCache {
    fn read(&self, image: &ImageInfo) -> Option<ImageStats> {
        self.0
            .get(&(image.buffer_content_hash, image.kind))
            .copied()
    }

    fn compute(&mut self, image: &ImageInfo) -> ImageStats {
        self.entry(image)
    }
}

impl Cache for ImageStatsCache {
    fn name(&self) -> &'static str {
        "ImageStatsCache"
    }

    fn purge_memory(&mut self) {
        self.0.clear();
    }

    fn on_store_events(&mut self, events: &[&ChunkStoreEvent], _entity_db: &EntityDb) {
        re_tracing::profile_function!();

        let cache_key_removed = filter_blob_removed_events(events);
        self.0
            .retain(|(cache_key, _kind), _stats| !cache_key_removed.contains(cache_key));
    }
}

impl AppCache for ImageStatsCache {}

impl re_byte_size::MemUsageTreeCapture for ImageStatsCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_byte_size::MemUsageTree::Bytes(self.0.total_size_bytes())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_chunk::{Chunk, RowId, TimePoint};
    use re_log_types::StoreId;
    use re_sdk_types::archetypes::Image;
    use re_sdk_types::encodings::{ChannelDatatype, ColorModel, ImageFormat};

    use super::*;

    /// Logs a static 2x2 grayscale image to `image`, forwards the store events to `cache`,
    /// and returns the logged image.
    fn log_static_image(entity_db: &mut EntityDb, cache: &mut ImageStatsCache) -> ImageInfo {
        let pixels = vec![0, 64, 128, 255];
        let row_id = RowId::new();
        let chunk = Chunk::builder("image")
            .with_archetype(
                row_id,
                TimePoint::STATIC,
                &Image::from_l8(pixels.clone(), [2, 2]),
            )
            .build()
            .expect("building a chunk with a single image should succeed");
        let events = entity_db
            .add_chunk(&Arc::new(chunk))
            .expect("adding a chunk should succeed");
        cache.on_store_events(&events.iter().collect::<Vec<_>>(), entity_db);

        ImageInfo::from_stored_blob(
            row_id,
            Image::descriptor_buffer().component,
            pixels.into(),
            ImageFormat::from_color_model([2, 2], ColorModel::L, ChannelDatatype::U8),
            ImageKind::Color,
        )
    }

    /// Overwriting a static raw image evicts the stats of the old image (rerun-io/rerun#13039).
    #[test]
    fn overwritten_static_image_stats_are_evicted() {
        let mut entity_db = EntityDb::new(StoreId::recording("test_app", "test_recording"));
        let mut cache = ImageStatsCache::default();

        let old_image = log_static_image(&mut entity_db, &mut cache);
        cache.entry(&old_image);

        log_static_image(&mut entity_db, &mut cache);

        assert!(
            cache.read(&old_image).is_none(),
            "stats of the overwritten image should be evicted"
        );
    }
}
