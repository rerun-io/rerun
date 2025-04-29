use ahash::{HashMap, HashSet};
use itertools::Either;

use re_chunk_store::ChunkStoreEvent;
use re_log_types::hash::Hash64;
use re_types::{
    components::{ImageBuffer, MediaType},
    image::{ImageKind, ImageLoadError},
    Component as _,
};

use crate::{Cache, ImageInfo};

struct DecodedImageResult {
    /// Cached `Result` from decoding the image
    result: Result<ImageInfo, ImageLoadError>,

    /// Total memory used by this image.
    memory_used: u64,

    /// At which [`ImageDecodeCache::generation`] was this image last used?
    last_use_generation: u64,
}

/// Caches the results of decoding [`re_types::archetypes::EncodedImage`].
#[derive(Default)]
pub struct ImageDecodeCache {
    cache: HashMap<Hash64, HashMap<Hash64, DecodedImageResult>>,
    memory_used: u64,
    generation: u64,
}

#[allow(clippy::map_err_ignore)]
impl ImageDecodeCache {
    /// Decode some image data and cache the result.
    ///
    /// The `RowId`, if available, may be used to generate the cache key.
    /// NOTE: images are never batched atm (they are mono-archetypes),
    /// so we don't need the instance id here.
    pub fn entry(
        &mut self,
        blob_cache_key: Hash64,
        image_bytes: &[u8],
        media_type: Option<&MediaType>,
    ) -> Result<ImageInfo, ImageLoadError> {
        re_tracing::profile_function!();

        // In order to avoid loading the same video multiple times with
        // known and unknown media type, we have to resolve the media type before
        // loading & building the cache key.
        let Some(media_type) = media_type
            .cloned()
            .or_else(|| MediaType::guess_from_data(image_bytes))
        else {
            return Err(ImageLoadError::UnrecognizedMimeType);
        };

        let inner_key = Hash64::hash(&media_type);

        let lookup = self
            .cache
            .entry(blob_cache_key)
            .or_default()
            .entry(inner_key)
            .or_insert_with(|| {
                let result = decode_image(blob_cache_key, image_bytes, media_type.as_str());
                let memory_used = result.as_ref().map_or(0, |image| image.buffer.len() as u64);
                self.memory_used += memory_used;
                DecodedImageResult {
                    result,
                    memory_used,
                    last_use_generation: 0,
                }
            });
        lookup.last_use_generation = self.generation;
        lookup.result.clone()
    }
}

fn decode_image(
    blob_cache_key: Hash64,
    image_bytes: &[u8],
    media_type: &str,
) -> Result<ImageInfo, ImageLoadError> {
    re_tracing::profile_function!();

    let mut reader = image::ImageReader::new(std::io::Cursor::new(image_bytes));

    if let Some(format) = image::ImageFormat::from_mime_type(media_type) {
        reader.set_format(format);
    } else {
        return Err(ImageLoadError::UnsupportedMimeType(media_type.to_owned()));
    }

    let dynamic_image = reader.decode()?;

    let (buffer, format) = ImageBuffer::from_dynamic_image(dynamic_image)?;

    Ok(ImageInfo {
        buffer_cache_key: blob_cache_key,
        buffer: buffer.0,
        format: format.0,
        kind: ImageKind::Color,
    })
}

impl Cache for ImageDecodeCache {
    fn begin_frame(&mut self, _renderer_active_frame_idx: u64) {
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
                        .contains_component_name(re_types::components::Blob::name())
                };

                if is_deletion() && contains_image_blob() {
                    Either::Left(event.chunk.row_ids().map(Hash64::hash))
                } else {
                    Either::Right(std::iter::empty())
                }
            })
            .collect();

        self.cache
            .retain(|cache_key, _per_key| !cache_key_removed.contains(cache_key));
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
