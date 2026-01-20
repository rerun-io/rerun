use ahash::HashMap;

use re_chunk::RowId;
use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;
use re_log_types::hash::Hash64;
use re_rvl::{RosRvlMetadata, decode_rvl_with_quantization};
use re_sdk_types::ComponentIdentifier;
use re_sdk_types::components::{ImageBuffer, ImageFormat as ImageFormatComponent, MediaType};
use re_sdk_types::datatypes::{Blob, ChannelDatatype, ColorModel, ImageFormat};
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

    /// Decode some depth image data and cache the result.
    ///
    /// The `RowId`, if available, may be used to generate the cache key.
    /// NOTE: depth images are never batched atm (they are mono-archetypes),
    /// so we don't need the instance id here.
    pub fn entry_encoded_depth(
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
            decode_encoded_depth(
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
    re_tracing::profile_function!();

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

fn decode_encoded_depth(
    blob_row_id: RowId,
    blob_component: ComponentIdentifier,
    image_bytes: &[u8],
    media_type: &str,
) -> Result<ImageInfo, ImageLoadError> {
    match media_type {
        MediaType::PNG => decode_png_depth(blob_row_id, blob_component, image_bytes),
        MediaType::RVL => decode_rvl_depth(blob_row_id, blob_component, image_bytes),
        other => Err(ImageLoadError::UnsupportedMimeType(other.to_owned())),
    }
}

fn decode_png_depth(
    blob_row_id: RowId,
    blob_component: ComponentIdentifier,
    image_bytes: &[u8],
) -> Result<ImageInfo, ImageLoadError> {
    re_tracing::profile_function!();

    let mut reader = image::ImageReader::new(std::io::Cursor::new(image_bytes));
    reader.set_format(image::ImageFormat::Png);

    let dynamic_image = reader.decode()?;
    let (buffer, mut format) = ImageBuffer::from_dynamic_image(dynamic_image)?;

    if format.color_model != Some(ColorModel::L) {
        return Err(ImageLoadError::DecodeError(format!(
            "Encoded depth PNG must be single-channel (L); got {:?}",
            format.color_model
        )));
    }
    // .. but in our semantics we treat depth as `None` color model since there _is_ no color. (see `ImageKind::Depth`)
    format.color_model = None;

    let expected_num_bytes = format.num_bytes();
    let ImageBuffer(blob) = buffer;
    let actual_num_bytes = blob.len();
    if actual_num_bytes != expected_num_bytes {
        return Err(ImageLoadError::DecodeError(format!(
            "Encoded depth PNG payload is {actual_num_bytes} B, but {format:?} requires {expected_num_bytes} B",
        )));
    }

    Ok(ImageInfo::from_stored_blob(
        blob_row_id,
        blob_component,
        blob,
        *format,
        ImageKind::Depth,
    ))
}

fn decode_rvl_depth(
    blob_row_id: RowId,
    blob_component: ComponentIdentifier,
    image_bytes: &[u8],
) -> Result<ImageInfo, ImageLoadError> {
    let metadata = RosRvlMetadata::parse(image_bytes)
        .map_err(|err| ImageLoadError::DecodeError(err.to_string()))?;

    let format = ImageFormatComponent::from(ImageFormat::depth(
        [metadata.width, metadata.height],
        ChannelDatatype::F32, // We always use the quantization information from the metadata to convert to f32.
    ));

    let buffer: Vec<u8> = decode_rvl_with_quantization(image_bytes, &metadata)
        .map(|v| {
            bytemuck::try_cast_vec(v).unwrap_or_else(|(_err, v)| bytemuck::cast_slice(&v).to_vec())
        })
        .map_err(|err| ImageLoadError::DecodeError(err.to_string()))?;

    let expected_num_bytes = format.num_bytes();
    let actual_num_bytes = buffer.len();
    if actual_num_bytes != expected_num_bytes {
        return Err(ImageLoadError::DecodeError(format!(
            "RVL payload decoded to {actual_num_bytes} B, but {format:?} requires {expected_num_bytes} B"
        )));
    }

    Ok(ImageInfo::from_stored_blob(
        blob_row_id,
        blob_component,
        Blob::from(buffer),
        format.0,
        ImageKind::Depth,
    ))
}

impl Cache for ImageDecodeCache
where
    // NOTE: Explicit bounds help the compiler avoid recursion overflow when checking trait implementations.
    ImageInfo: Send + Sync,
    ImageLoadError: Send + Sync,
{
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

impl re_byte_size::MemUsageTreeCapture for ImageDecodeCache
where
    // NOTE: Explicit bounds help the compiler avoid recursion overflow when checking trait implementations.
    ImageInfo: Send + Sync,
    ImageLoadError: Send + Sync,
{
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

#[cfg(test)]
mod tests {
    use super::*;

    use image::{ColorType, ImageEncoder as _, codecs::png::PngEncoder};

    #[test]
    fn entry_encoded_depth_guesses_png_media_type() {
        let width = 2;
        let height = 2;
        let depth_values: [u16; 4] = [0, 1, 2, 3];

        let mut encoded_png = Vec::new();
        {
            let encoder = PngEncoder::new(&mut encoded_png);
            encoder
                .write_image(
                    bytemuck::cast_slice(&depth_values),
                    width,
                    height,
                    ColorType::L16.into(),
                )
                .expect("encoding png failed");
        }

        let mut cache = ImageDecodeCache::default();

        let image_info = cache
            .entry_encoded_depth(
                RowId::ZERO,
                ComponentIdentifier::from("test"),
                &encoded_png,
                None,
            )
            .expect("decoding encoded depth image failed");

        assert_eq!(image_info.kind, ImageKind::Depth);
        assert_eq!(
            image_info.format,
            ImageFormat::depth([width, height], ChannelDatatype::U16)
        );
    }

    #[test]
    fn decoding_png_depth_works() {
        let width = 2;
        let height = 2;
        let depth_values: [u16; 4] = [0, 1, 2, 3];

        let mut encoded_png = Vec::new();
        {
            let encoder = PngEncoder::new(&mut encoded_png);
            encoder
                .write_image(
                    bytemuck::cast_slice(&depth_values),
                    width,
                    height,
                    ColorType::L16.into(),
                )
                .expect("encoding png failed");
        }

        let image_info =
            decode_png_depth(RowId::ZERO, ComponentIdentifier::from("test"), &encoded_png)
                .expect("decoding png depth failed");

        assert_eq!(image_info.kind, ImageKind::Depth);
        assert_eq!(
            image_info.format,
            ImageFormat::depth([width, height], ChannelDatatype::U16)
        );
        assert_eq!(
            image_info.buffer.len(),
            depth_values.len() * std::mem::size_of::<u16>()
        );
    }
}
