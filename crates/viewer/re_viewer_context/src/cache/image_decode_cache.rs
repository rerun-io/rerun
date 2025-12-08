use ahash::HashMap;
use bytemuck::Pod;

use re_chunk::RowId;
use re_chunk_store::ChunkStoreEvent;
use re_depth_compression::ros_rvl::{
    decode_ros_rvl_f32, decode_ros_rvl_u16, parse_ros_rvl_metadata,
};
use re_entity_db::EntityDb;
use re_log_types::hash::Hash64;
use re_sdk_types::ComponentIdentifier;
use re_sdk_types::components::{ImageBuffer, ImageFormat as ImageFormatComponent, MediaType};
use re_sdk_types::datatypes::{Blob, ChannelDatatype, ColorModel};
use re_sdk_types::image::{ImageKind, ImageLoadError};

use crate::cache::filter_blob_removed_events;
use crate::image_info::StoredBlobCacheKey;
use crate::{Cache, CacheMemoryReport, CacheMemoryReportItem, ImageInfo};

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
    pub fn entry(
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

    pub fn entry_encoded_depth(
        &mut self,
        blob_row_id: RowId,
        blob_component: ComponentIdentifier,
        image_bytes: &[u8],
        media_type: Option<&MediaType>,
        format: &ImageFormatComponent,
    ) -> Result<ImageInfo, ImageLoadError> {
        re_tracing::profile_function!();

        let Some(media_type) = media_type
            .cloned()
            .or_else(|| MediaType::guess_from_data(image_bytes))
        else {
            return Err(ImageLoadError::UnrecognizedMimeType);
        };

        let inner_key = Hash64::hash(&(media_type.clone(), *format));

        self.cache_lookup_or_decode(blob_row_id, blob_component, inner_key, || {
            decode_encoded_depth(
                blob_row_id,
                blob_component,
                image_bytes,
                media_type.as_str(),
                format,
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
    format: &ImageFormatComponent,
) -> Result<ImageInfo, ImageLoadError> {
    match media_type {
        MediaType::PNG => decode_png_depth(blob_row_id, blob_component, image_bytes, format),
        MediaType::RVL => decode_rvl_depth(blob_row_id, blob_component, image_bytes, format),
        other => Err(ImageLoadError::UnsupportedMimeType(other.to_owned())),
    }
}

fn decode_png_depth(
    blob_row_id: RowId,
    blob_component: ComponentIdentifier,
    image_bytes: &[u8],
    format: &ImageFormatComponent,
) -> Result<ImageInfo, ImageLoadError> {
    re_tracing::profile_function!();

    let mut reader = image::ImageReader::new(std::io::Cursor::new(image_bytes));
    reader.set_format(image::ImageFormat::Png);

    let dynamic_image = reader.decode()?;

    if dynamic_image.width() != format.width || dynamic_image.height() != format.height {
        return Err(ImageLoadError::DecodeError(format!(
            "Encoded depth PNG resolution mismatch: blob is {}x{}, expected {}x{}",
            dynamic_image.width(),
            dynamic_image.height(),
            format.width,
            format.height
        )));
    }

    let (buffer, decoded_format) = ImageBuffer::from_dynamic_image(dynamic_image)?;

    if decoded_format.color_model != Some(ColorModel::L) {
        return Err(ImageLoadError::DecodeError(format!(
            "Encoded depth PNG must be single-channel (L); got {:?}",
            decoded_format.color_model
        )));
    }

    if decoded_format.datatype() != format.datatype() {
        return Err(ImageLoadError::DecodeError(format!(
            "Encoded depth PNG datatype mismatch: blob is {:?}, expected {:?}",
            decoded_format.datatype(),
            format.datatype()
        )));
    }

    let expected_num_bytes = format.num_bytes();
    let ImageBuffer(blob) = buffer;
    let actual_num_bytes = blob.len();
    if actual_num_bytes != expected_num_bytes {
        return Err(ImageLoadError::DecodeError(format!(
            "Encoded depth PNG payload is {actual_num_bytes} B, but {} requires {expected_num_bytes} B",
            format.0
        )));
    }

    Ok(ImageInfo::from_stored_blob(
        blob_row_id,
        blob_component,
        blob,
        format.0,
        ImageKind::Depth,
    ))
}

fn decode_rvl_depth(
    blob_row_id: RowId,
    blob_component: ComponentIdentifier,
    image_bytes: &[u8],
    format: &ImageFormatComponent,
) -> Result<ImageInfo, ImageLoadError> {
    let metadata = parse_ros_rvl_metadata(image_bytes)
        .map_err(|err| ImageLoadError::DecodeError(err.to_string()))?;

    let expected_pixels = (format.width as usize) * (format.height as usize);
    if metadata.num_pixels() != expected_pixels {
        return Err(ImageLoadError::DecodeError(format!(
            "RVL encoded depth metadata {metadata_width}x{metadata_height} disagrees with ImageFormat {}x{}",
            format.width,
            format.height,
            metadata_width = metadata.width,
            metadata_height = metadata.height
        )));
    }

    let buffer: Vec<u8> = match format.datatype() {
        ChannelDatatype::U16 => decode_ros_rvl_u16(image_bytes, &metadata)
            .map(|x: Vec<u16>| vec_into_bytes(&x))
            .map_err(|err| ImageLoadError::DecodeError(err.to_string()))?,
        ChannelDatatype::F32 => decode_ros_rvl_f32(image_bytes, &metadata)
            .map(|x: Vec<f32>| vec_into_bytes(&x))
            .map_err(|err| ImageLoadError::DecodeError(err.to_string()))?,
        other => {
            return Err(ImageLoadError::DecodeError(format!(
                "Unsupported RVL channel datatype {other:?}"
            )));
        }
    };

    let expected_num_bytes = format.num_bytes();
    let actual_num_bytes = buffer.len();
    if actual_num_bytes != expected_num_bytes {
        return Err(ImageLoadError::DecodeError(format!(
            "RVL payload decoded to {actual_num_bytes} B, but {} requires {expected_num_bytes} B",
            format.0
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

fn vec_into_bytes<T: Pod>(vec: &[T]) -> Vec<u8> {
    bytemuck::cast_slice(vec).to_vec()
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

    fn memory_report(&self) -> CacheMemoryReport {
        let mut items: Vec<_> = self
            .cache
            .iter()
            .map(|(k, images)| CacheMemoryReportItem {
                item_name: format!("{:x}", k.0.hash64()),
                bytes_cpu: images.values().map(|image| image.memory_used).sum(),
                bytes_gpu: None,
            })
            .collect();
        items.sort_by(|a, b| a.item_name.cmp(&b.item_name));
        CacheMemoryReport {
            bytes_cpu: self.memory_used,
            bytes_gpu: None,
            per_cache_item_info: items,
        }
    }

    fn name(&self) -> &'static str {
        "Image Decodings"
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

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use image::{ColorType, ImageEncoder as _, codecs::png::PngEncoder};
    use re_sdk_types::datatypes::ImageFormat as ImageFormatDatatype;

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

        let format = ImageFormatComponent::from(ImageFormatDatatype::depth(
            [width, height],
            ChannelDatatype::U16,
        ));

        let mut cache = ImageDecodeCache::default();

        let image_info = cache
            .entry_encoded_depth(
                RowId::ZERO,
                ComponentIdentifier::from("test"),
                &encoded_png,
                None,
                &format,
            )
            .expect("decoding encoded depth image failed");

        assert_eq!(image_info.kind, ImageKind::Depth);
        assert_eq!(image_info.format, format.0);
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

        let format = ImageFormatComponent::from(ImageFormatDatatype::depth(
            [width, height],
            ChannelDatatype::U16,
        ));

        let image_info = decode_png_depth(
            RowId::ZERO,
            ComponentIdentifier::from("test"),
            &encoded_png,
            &format,
        )
        .expect("decoding png depth failed");

        assert_eq!(image_info.kind, ImageKind::Depth);
        assert_eq!(image_info.format, format.0);
        assert_eq!(
            image_info.buffer.len(),
            depth_values.len() * std::mem::size_of::<u16>()
        );
    }
}
