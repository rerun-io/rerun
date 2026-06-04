use crate::cache::filter_blob_removed_events;
use crate::image_info::StoredBlobCacheKey;
use crate::{Cache, CacheEntryAccess, ImageInfo};
use ahash::HashMap;
use re_byte_size::SizeBytes as _;
use re_byte_size::{MemUsageTree, MemUsageTreeCapture};
use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;
use std::sync::Arc;

/// Per-channel histogram of an 8-bit `RGB` image.
///
/// Each channel has 256 bins.
#[derive(Clone, Debug, re_byte_size::SizeBytes)]
pub struct Rgb8Histogram {
    /// One 256-bin histogram per channel (R, G, B).
    pub bins: [[u64; 256]; 3],
}

impl Rgb8Histogram {
    /// Compute the per-channel histogram of an 8-bit `RGB` buffer.
    pub fn from_rgb8(rgb: &[u8]) -> Self {
        re_tracing::profile_function!();

        let mut bins_r = [0_u64; 256];
        let mut bins_g = [0_u64; 256];
        let mut bins_b = [0_u64; 256];

        let (chunks, _remainder) = rgb.as_chunks::<3>();
        for &[r, g, b] in chunks {
            bins_r[r as usize] += 1;
            bins_g[g as usize] += 1;
            bins_b[b as usize] += 1;
        }

        Self {
            bins: [bins_r, bins_g, bins_b],
        }
    }
}

/// Caches per-channel histograms for 8-bit RGB images, keyed by image content.
#[derive(Default)]
pub struct ImageHistogramCache(HashMap<StoredBlobCacheKey, Arc<Rgb8Histogram>>);

impl ImageHistogramCache {
    /// Get the histogram for the given 8-bit `RGB` image, computing and caching it on first access.
    ///
    /// The caller is responsible for only passing in 8-bit RGB images.
    pub fn entry(&mut self, image: &ImageInfo) -> Arc<Rgb8Histogram> {
        self.0
            .entry(image.buffer_content_hash)
            .or_insert_with(|| Arc::new(Rgb8Histogram::from_rgb8(&image.buffer)))
            .clone()
    }
}

impl CacheEntryAccess<ImageInfo, Arc<Rgb8Histogram>> for ImageHistogramCache {
    fn read(&self, image: &ImageInfo) -> Option<Arc<Rgb8Histogram>> {
        self.0.get(&image.buffer_content_hash).cloned()
    }

    fn compute(&mut self, image: &ImageInfo) -> Arc<Rgb8Histogram> {
        self.entry(image)
    }
}

impl Cache for ImageHistogramCache {
    fn name(&self) -> &'static str {
        "ImageHistogramCache"
    }

    fn purge_memory(&mut self) {
        // [[u64; 256]; 3] ≈ 6 KiB per cached image — small enough that we
        // leave it to store-event invalidation rather than periodic purging.
    }

    fn on_store_events(&mut self, events: &[&ChunkStoreEvent], _entity_db: &EntityDb) {
        let removed = filter_blob_removed_events(events);
        if removed.is_empty() {
            return;
        }
        self.0.retain(|key, _| !removed.contains(key));
    }
}

impl MemUsageTreeCapture for ImageHistogramCache {
    fn capture_mem_usage_tree(&self) -> MemUsageTree {
        MemUsageTree::Bytes(self.0.total_size_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_buffer_yields_zero_bins() {
        let hist = Rgb8Histogram::from_rgb8(&[]);
        for channel in &hist.bins {
            assert!(channel.iter().all(|&c| c == 0));
        }
    }

    #[test]
    fn single_pixel_increments_one_bin_per_channel() {
        let hist = Rgb8Histogram::from_rgb8(&[10, 20, 30]);
        assert_eq!(hist.bins[0][10], 1);
        assert_eq!(hist.bins[1][20], 1);
        assert_eq!(hist.bins[2][30], 1);
        let total: u64 = hist.bins.iter().flatten().sum();
        assert_eq!(total, 3);
    }

    #[test]
    fn trailing_bytes_are_ignored() {
        // 4 trailing bytes; only the first complete pixel should contribute.
        let hist = Rgb8Histogram::from_rgb8(&[1, 2, 3, 99]);
        assert_eq!(hist.bins[0][1], 1);
        assert_eq!(hist.bins[1][2], 1);
        assert_eq!(hist.bins[2][3], 1);
        assert_eq!(hist.bins[0][99], 0);
        assert_eq!(hist.bins[1][99], 0);
        assert_eq!(hist.bins[2][99], 0);
    }

    #[test]
    fn many_pixels_count_correctly() {
        // 100 pixels, all (5, 6, 7).
        let buffer: Vec<u8> = (0..100).flat_map(|_| [5, 6, 7]).collect();
        let hist = Rgb8Histogram::from_rgb8(&buffer);
        assert_eq!(hist.bins[0][5], 100);
        assert_eq!(hist.bins[1][6], 100);
        assert_eq!(hist.bins[2][7], 100);
        // No other bins should be set.
        for channel in 0..3 {
            for bin in 0..256 {
                let expected = match (channel, bin) {
                    (0, 5) | (1, 6) | (2, 7) => 100,
                    _ => 0,
                };
                assert_eq!(
                    hist.bins[channel][bin], expected,
                    "channel {channel} bin {bin}"
                );
            }
        }
    }
}
