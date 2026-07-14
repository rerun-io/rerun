use ahash::HashMap;

use re_chunk::RowId;
use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;
use re_log_types::hash::Hash64;
use re_sdk_types::ComponentIdentifier;
use re_sdk_types::components::{ImageBuffer, MediaType};
use re_sdk_types::image::{ImageKind, ImageLoadError};

use crate::cache::filter_blob_removed_events;
use crate::image_info::StoredBlobCacheKey;
use crate::{Cache, ImageInfo};

struct DecodedImageResult {
    /// Cached `Result` from decoding the image
    result: Result<ImageInfo, ImageLoadError>,

    /// Total memory used by this image.
    memory_used: u64,

    /// At which [`ImageDecodeCache::generation`] was this image last used?
    last_use_generation: u64,
}

/// Caches the results of decoding [`re_sdk_types::archetypes::EncodedImage`] and [`re_sdk_types::archetypes::EncodedDepthImage`].
#[derive(Default)]
pub struct ImageDecodeCache {
    cache: HashMap<StoredBlobCacheKey, HashMap<Hash64, DecodedImageResult>>,
    memory_used: u64,
    generation: u64,
}

impl ImageDecodeCache {
    // TODO(RR-4570): Remove this, we want to use the video player instead of this
    //             but hard to do for the only remaining usage in `redap_thumbnail`.
    #[deprecated = "Use video stream cache instead if possible."]
    /// Decode some image data and cache the result.
    ///
    /// The `RowId`, if available, may be used to generate the cache key.
    /// NOTE: images are never batched atm (they are mono-archetypes),
    /// so we don't need the instance id here.
    pub fn entry_encoded_color(
        &mut self,
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

        let inner_key = Hash64::hash(&media_type);

        self.cache_lookup_or_decode(blob_row_id, blob_component, inner_key, || {
            decode_color_image(
                blob_row_id,
                blob_component,
                image_bytes,
                media_type.as_str(),
            )
        })
    }

    fn cache_lookup_or_decode<F>(
        &mut self,
        blob_row_id: RowId,
        blob_component: ComponentIdentifier,
        cache_key: Hash64,
        decode: F,
    ) -> Result<ImageInfo, ImageLoadError>
    where
        F: FnOnce() -> Result<ImageInfo, ImageLoadError>,
    {
        let blob_cache_key = StoredBlobCacheKey::new(blob_row_id, blob_component);

        if let Some(existing) = self
            .cache
            .get_mut(&blob_cache_key)
            .and_then(|per_blob| per_blob.get_mut(&cache_key))
        {
            existing.last_use_generation = self.generation;
            return existing.result.clone();
        }

        let result = decode();
        let memory_used = result.as_ref().map_or(0, |image| image.buffer.len() as u64);
        self.memory_used += memory_used;

        self.cache.entry(blob_cache_key).or_default().insert(
            cache_key,
            DecodedImageResult {
                result: result.clone(),
                memory_used,
                last_use_generation: self.generation,
            },
        );

        result
    }
}

fn decode_color_image(
    blob_row_id: RowId,
    blob_component: ComponentIdentifier,
    image_bytes: &[u8],
    media_type: &str,
) -> Result<ImageInfo, ImageLoadError> {
    re_tracing::profile_function!(media_type);

    let mut reader = image::ImageReader::new(std::io::Cursor::new(image_bytes));

    if let Some(format) = image::ImageFormat::from_mime_type(media_type) {
        reader.set_format(format);
    } else {
        return Err(ImageLoadError::UnsupportedMimeType(media_type.to_owned()));
    }

    let dynamic_image = reader.decode()?;

    let (buffer, format) = ImageBuffer::from_dynamic_image(dynamic_image)?;

    Ok(ImageInfo::from_stored_blob(
        blob_row_id,
        blob_component,
        buffer.0,
        format.0,
        ImageKind::Color,
    ))
}

impl Cache for ImageDecodeCache {
    fn name(&self) -> &'static str {
        "ImageDecodeCache"
    }

    fn begin_frame(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        let max_decode_cache_use = 4_000_000_000;

        #[cfg(target_arch = "wasm32")]
        let max_decode_cache_use = 1_000_000_000;

        // TODO(jleibs): a more incremental purging mechanism, maybe switching to an LRU Cache
        // would likely improve the behavior.

        if self.memory_used > max_decode_cache_use {
            self.purge_memory();
        }

        self.generation += 1;
    }

    fn purge_memory(&mut self) {
        re_tracing::profile_function!();

        // Very aggressively flush everything not used in this frame

        let before = self.memory_used;

        self.cache.retain(|_cache_key, per_key| {
            per_key.retain(|_, ci| {
                let retain = ci.last_use_generation == self.generation;
                if !retain {
                    self.memory_used -= ci.memory_used;
                }
                retain
            });

            !per_key.is_empty()
        });

        re_log::trace!(
            "Flushed tensor decode cache. Before: {:.2} GB. After: {:.2} GB",
            before as f64 / 1e9,
            self.memory_used as f64 / 1e9,
        );
    }

    fn on_store_events(&mut self, events: &[&ChunkStoreEvent], _entity_db: &EntityDb) {
        re_tracing::profile_function!();

        let cache_key_removed = filter_blob_removed_events(events);
        self.cache
            .retain(|cache_key, _per_key| !cache_key_removed.contains(cache_key));
    }
}

impl re_byte_size::MemUsageTreeCapture for ImageDecodeCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        let mut node = re_byte_size::MemUsageNode::new();

        // Add per-item breakdown
        let mut items: Vec<_> = self
            .cache
            .iter()
            .map(|(k, images)| {
                let bytes_cpu: u64 = images.values().map(|image| image.memory_used).sum();
                (format!("{:x}", k.0.hash64()), bytes_cpu)
            })
            .collect();
        items.sort_by(|a, b| a.0.cmp(&b.0));

        for (item_name, bytes_cpu) in items {
            node.add(item_name, re_byte_size::MemUsageTree::Bytes(bytes_cpu));
        }

        node.into_tree()
    }
}
