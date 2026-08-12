use ahash::HashMap;

use re_byte_size::SizeBytes as _;
use re_chunk::RowId;
use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;
use re_log_types::hash::Hash64;
use re_sdk_types::ComponentIdentifier;
use re_sdk_types::components::MediaType;
use re_sdk_types::image::{ImageKind, ImageLoadError};

use crate::cache::filter_blob_removed_events;
use crate::image_info::StoredBlobCacheKey;
use crate::{Cache, ImageInfo, ImageStats};

/// Caches the [`ImageStats`] of encoded depth images
/// ([`re_sdk_types::archetypes::EncodedDepthImage`]), e.g. to derive a depth range.
///
/// The image is decoded transiently. Only the stats are retained, not the decoded pixels.
/// Blobs that failed to decode are cached as `None`, so decoding is not retried every frame.
// TODO(RR-4570): Ideally the stats would be derived from the frames the video player
//             decodes anyway, instead of decoding a second time here.
#[derive(Default)]
pub struct EncodedDepthImageStatsCache(
    // The inner key is the hash of the media type,
    // since a media type logged later can change how the same blob is decoded.
    HashMap<StoredBlobCacheKey, HashMap<Hash64, Option<ImageStats>>>,
);

impl EncodedDepthImageStatsCache {
    /// Decode some depth image data (e.g. 16-bit PNG or RVL) and compute & cache its stats.
    ///
    /// NOTE: images are never batched atm (they are mono-archetypes),
    /// so we don't need the instance id here.
    pub fn entry(
        &mut self,
        blob_row_id: RowId,
        blob_component: ComponentIdentifier,
        image_bytes: &[u8],
        media_type: Option<&MediaType>,
    ) -> Option<ImageStats> {
        re_tracing::profile_function!();

        *self
            .0
            .entry(StoredBlobCacheKey::new(blob_row_id, blob_component))
            .or_default()
            .entry(Hash64::hash(media_type))
            .or_insert_with(|| {
                match decode_depth_image(blob_row_id, blob_component, image_bytes, media_type) {
                    Ok(image) => Some(ImageStats::from_image(&image)),
                    Err(err) => {
                        re_log::warn_once!("Failed to decode depth image: {err}");
                        None
                    }
                }
            })
    }
}

fn decode_depth_image(
    blob_row_id: RowId,
    blob_component: ComponentIdentifier,
    image_bytes: &[u8],
    media_type: Option<&MediaType>,
) -> Result<ImageInfo, ImageLoadError> {
    re_tracing::profile_function!();

    let Some(media_type) = media_type
        .cloned()
        .or_else(|| MediaType::guess_from_data(image_bytes))
    else {
        return Err(ImageLoadError::UnrecognizedMimeType);
    };

    if media_type.as_str() == MediaType::RVL {
        let metadata = re_rvl::RosRvlMetadata::parse(image_bytes)
            .map_err(|err| ImageLoadError::DecodeError(err.to_string()))?;
        let depths = re_rvl::decode_rvl_with_quantization(image_bytes, &metadata)
            .map_err(|err| ImageLoadError::DecodeError(err.to_string()))?;

        let format = re_sdk_types::datatypes::ImageFormat::depth(
            [metadata.width, metadata.height],
            re_sdk_types::datatypes::ChannelDatatype::F32,
        );

        return Ok(ImageInfo::from_stored_blob(
            blob_row_id,
            blob_component,
            arrow::buffer::Buffer::from_vec(depths).into(),
            format,
            ImageKind::Depth,
        ));
    }

    super::image_decode_cache::decode_image(
        blob_row_id,
        blob_component,
        image_bytes,
        media_type.as_str(),
        ImageKind::Depth,
    )
}

impl Cache for EncodedDepthImageStatsCache {
    fn name(&self) -> &'static str {
        "EncodedDepthImageStatsCache"
    }

    fn purge_memory(&mut self) {
        self.0.clear();
    }

    fn on_store_events(&mut self, events: &[&ChunkStoreEvent], _entity_db: &EntityDb) {
        re_tracing::profile_function!();

        let cache_key_removed = filter_blob_removed_events(events);
        self.0
            .retain(|cache_key, _per_media_type| !cache_key_removed.contains(cache_key));
    }
}

impl re_byte_size::MemUsageTreeCapture for EncodedDepthImageStatsCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_byte_size::MemUsageTree::Bytes(self.0.total_size_bytes())
    }
}

#[cfg(test)]
mod tests {
    use re_sdk_types::archetypes::EncodedDepthImage;

    use super::*;

    fn blob_component() -> ComponentIdentifier {
        EncodedDepthImage::descriptor_blob().component
    }

    fn encode_l16_png(values: &[u16], width: u32, height: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut buf);
        image::ImageEncoder::write_image(
            encoder,
            bytemuck::cast_slice(values),
            width,
            height,
            image::ColorType::L16.into(),
        )
        .unwrap();
        buf
    }

    /// Regression test for RR-5172: the stats used by the depth range fallback must
    /// be derived from the decoded pixel values, not from the encoded bit depth
    /// (which would put the maximum at 65535 for a 16-bit PNG).
    #[test]
    fn stats_come_from_decoded_png_pixels() {
        let png = encode_l16_png(&[0, 1000, 4000, 2500], 2, 2);
        let mut cache = EncodedDepthImageStatsCache::default();

        let stats = cache
            .entry(
                RowId::new(),
                blob_component(),
                &png,
                Some(&MediaType::png()),
            )
            .expect("16-bit grayscale PNG should decode");

        assert_eq!(stats.finite_range, (0.0, 4000.0));
    }

    /// Without an explicit media type, the media type is guessed from the blob contents.
    #[test]
    fn guesses_media_type_from_blob() {
        let png = encode_l16_png(&[0, 4000], 2, 1);
        let mut cache = EncodedDepthImageStatsCache::default();

        let stats = cache
            .entry(RowId::new(), blob_component(), &png, None)
            .expect("PNG should be recognized without an explicit media type");

        assert_eq!(stats.finite_range, (0.0, 4000.0));
    }

    /// A failed decode under one media type must not shadow a later successful decode
    /// of the same blob under a corrected media type.
    #[test]
    fn failures_are_cached_per_media_type() {
        let png = encode_l16_png(&[0, 4000], 2, 1);
        let row_id = RowId::new();
        let mut cache = EncodedDepthImageStatsCache::default();

        // PNG bytes declared as RVL fail to decode…
        assert!(
            cache
                .entry(row_id, blob_component(), &png, Some(&MediaType::rvl()))
                .is_none()
        );
        // …and the failure itself is memoized…
        let per_media_type = &cache.0[&StoredBlobCacheKey::new(row_id, blob_component())];
        assert_eq!(per_media_type.len(), 1);
        assert!(per_media_type.values().all(Option::is_none));
        // …but the same blob decodes fine once the media type is corrected.
        assert!(
            cache
                .entry(row_id, blob_component(), &png, Some(&MediaType::png()))
                .is_some()
        );
    }
}
