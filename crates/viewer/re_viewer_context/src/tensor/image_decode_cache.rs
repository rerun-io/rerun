use re_chunk::RowId;
use re_types::{
    archetypes::Image,
    image::{ImageKind, ImageLoadError},
};

use egui::util::hash;

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
    cache: ahash::HashMap<u64, DecodedImageResult>,
    memory_used: u64,
    generation: u64,
}

#[allow(clippy::map_err_ignore)]
impl ImageDecodeCache {
    /// Decode some image data and cache the result.
    ///
    /// The `row_id` should be the `RowId` of the blob.
    /// NOTE: images are never batched atm (they are mono-archetypes),
    /// so we don't need the instance id here.
    pub fn entry(
        &mut self,
        row_id: RowId,
        image_bytes: &[u8],
        media_type: Option<&str>,
    ) -> Result<ImageInfo, ImageLoadError> {
        re_tracing::profile_function!();

        let key = hash((row_id, media_type));

        let lookup = self.cache.entry(key).or_insert_with(|| {
            let result = decode_image(row_id, image_bytes, media_type);
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
    row_id: RowId,
    image_bytes: &[u8],
    media_type: Option<&str>,
) -> Result<ImageInfo, ImageLoadError> {
    re_tracing::profile_function!();

    let mut reader = image::io::Reader::new(std::io::Cursor::new(image_bytes));

    if let Some(media_type) = media_type {
        if let Some(format) = image::ImageFormat::from_mime_type(media_type) {
            reader.set_format(format);
        } else {
            re_log::warn!("Unsupported image MediaType/MIME: {media_type:?}");
        }
    }

    if reader.format().is_none() {
        if let Ok(format) = image::guess_format(image_bytes) {
            // Weirdly enough, `reader.decode` doesn't do this for us.
            reader.set_format(format);
        }
    }

    let dynamic_image = reader.decode()?;

    let image_arch = Image::from_dynamic_image(dynamic_image)?;

    let Image { buffer, format, .. } = image_arch;

    Ok(ImageInfo {
        buffer_row_id: row_id,
        buffer: buffer.0,
        format: format.0,
        kind: ImageKind::Color,
        colormap: None,
    })
}

impl Cache for ImageDecodeCache {
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

        self.cache.retain(|_, ci| {
            let retain = ci.last_use_generation == self.generation;
            if !retain {
                self.memory_used -= ci.memory_used;
            }
            retain
        });

        re_log::trace!(
            "Flushed tensor decode cache. Before: {:.2} GB. After: {:.2} GB",
            before as f64 / 1e9,
            self.memory_used as f64 / 1e9,
        );
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
