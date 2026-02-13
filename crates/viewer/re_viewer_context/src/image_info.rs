use std::borrow::Cow;
use std::ops::RangeInclusive;

use re_chunk::RowId;
use re_log_types::hash::Hash64;
use re_sdk_types::components::{self, Colormap};
use re_sdk_types::datatypes::{Blob, ChannelDatatype, ColorModel, ImageFormat};
use re_sdk_types::image::{ImageKind, rgb_from_yuv};
use re_sdk_types::tensor_data::TensorElement;
use re_sdk_types::{ComponentIdentifier, archetypes};

/// Get a fallback resolution for an image on a specific entity.
pub fn resolution_of_image_at(
    ctx: &crate::ViewerContext<'_>,
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
) -> Option<components::Resolution> {
    re_tracing::profile_function!();

    let entity_db = ctx.recording();
    let storage_engine = entity_db.storage_engine();

    // Check what kind of non-encoded images were logged here, if any.
    // TODO(andreas): can we do this more efficiently?
    // TODO(andreas): doesn't take blueprint into account!
    let all_components = storage_engine
        .store()
        .all_components_for_entity(entity_path)?;
    let image_format_descr = all_components
        .get(&archetypes::Image::descriptor_format().component)
        .or_else(|| all_components.get(&archetypes::DepthImage::descriptor_format().component))
        .or_else(|| {
            all_components.get(&archetypes::SegmentationImage::descriptor_format().component)
        });

    if let Some((_, image_format)) = image_format_descr.and_then(|component| {
        entity_db.latest_at_component::<components::ImageFormat>(entity_path, query, *component)
    }) {
        // Normal `Image` archetype
        return Some(components::Resolution::from([
            image_format.width as f32,
            image_format.height as f32,
        ]));
    }

    // Check for an encoded image.
    if let Some(((_time, row_id), blob)) = entity_db
        .latest_at_component::<re_sdk_types::components::Blob>(
            entity_path,
            query,
            archetypes::EncodedImage::descriptor_blob().component,
        )
    {
        let media_type = entity_db
            .latest_at_component::<components::MediaType>(
                entity_path,
                query,
                archetypes::EncodedImage::descriptor_media_type().component,
            )
            .map(|(_, c)| c);

        let image = ctx
            .store_context
            .caches
            .entry(|c: &mut crate::ImageDecodeCache| {
                c.entry_encoded_color(
                    row_id,
                    archetypes::EncodedImage::descriptor_blob().component,
                    &blob,
                    media_type.as_ref(),
                )
            });

        if let Ok(image) = image {
            return Some(image.width_height_f32().into());
        }
    }

    // Check for an encoded depth image.
    if let Some(((_time, row_id), blob)) = entity_db
        .latest_at_component::<re_sdk_types::components::Blob>(
            entity_path,
            query,
            archetypes::EncodedDepthImage::descriptor_blob().component,
        )
    {
        let media_type = entity_db
            .latest_at_component::<components::MediaType>(
                entity_path,
                query,
                archetypes::EncodedDepthImage::descriptor_media_type().component,
            )
            .map(|(_, c)| c);

        let depth_image = ctx
            .store_context
            .caches
            .entry(|c: &mut crate::ImageDecodeCache| {
                c.entry_encoded_depth(
                    row_id,
                    archetypes::EncodedDepthImage::descriptor_blob().component,
                    &blob,
                    media_type.as_ref(),
                )
            });

        if let Ok(depth_image) = depth_image {
            return Some(depth_image.width_height_f32().into());
        }
    }

    None
}

/// Colormap together with the range of image values that is mapped to the colormap's range.
///
/// The range is used to linearly re-map the image values to a normalized range (of 0-1)
/// to which the colormap is applied.
#[derive(Clone)]
pub struct ColormapWithRange {
    pub colormap: Colormap,
    pub value_range: [f32; 2],
}

impl ColormapWithRange {
    pub const DEFAULT_DEPTH_COLORMAP: Colormap = Colormap::Turbo;

    pub fn default_range_for_depth_images(image_stats: &crate::ImageStats) -> [f32; 2] {
        // Use 0.0 as default minimum depth value, even if it doesn't show up in the data.
        // (since logically, depth usually starts at zero)
        [0.0, image_stats.finite_range.1 as _]
    }

    pub fn default_for_depth_images(image_stats: &crate::ImageStats) -> Self {
        Self {
            colormap: Self::DEFAULT_DEPTH_COLORMAP,
            value_range: Self::default_range_for_depth_images(image_stats),
        }
    }
}

/// Hash used for identifying blobs stored in a store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StoredBlobCacheKey(pub Hash64);

impl re_byte_size::SizeBytes for StoredBlobCacheKey {
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    fn is_pod() -> bool {
        true
    }
}

impl StoredBlobCacheKey {
    pub const ZERO: Self = Self(Hash64::ZERO);

    pub fn new(blob_row_id: RowId, component: ComponentIdentifier) -> Self {
        // Row ID + component is enough because in a single row & column there
        // can currently only be a single blob since blobs are internally stored as transparent dynamic byte arrays.
        Self(Hash64::hash((blob_row_id, component)))
    }
}

/// Represents the contents of an `Image`, `SegmentationImage` or `DepthImage`.
#[derive(Clone)]
pub struct ImageInfo {
    /// Hash for the contents of the blob.
    ///
    /// This does **not** need to take into account the image format.
    pub buffer_content_hash: StoredBlobCacheKey,

    /// The image data, row-wise, with stride=width.
    pub buffer: Blob,

    /// Describes the format of [`Self::buffer`].
    pub format: ImageFormat,

    /// Color, Depth, or Segmentation?
    pub kind: ImageKind,
}

impl ImageInfo {
    pub fn from_stored_blob(
        blob_row_id: RowId,
        component: ComponentIdentifier,
        buffer: Blob,
        format: ImageFormat,
        kind: ImageKind,
    ) -> Self {
        Self {
            buffer_content_hash: StoredBlobCacheKey::new(blob_row_id, component),
            buffer,
            format,
            kind,
        }
    }

    #[inline]
    pub fn width(&self) -> u32 {
        self.format.width
    }

    #[inline]
    pub fn height(&self) -> u32 {
        self.format.height
    }

    pub fn width_height(&self) -> [u32; 2] {
        [self.format.width, self.format.height]
    }

    pub fn width_height_f32(&self) -> [f32; 2] {
        [self.format.width as f32, self.format.height as f32]
    }

    /// Returns [`ColorModel::L`] for depth and segmentation images.
    ///
    /// Currently return [`ColorModel::RGB`] for chroma-subsampled images,
    /// but this may change in the future when we add YUV support to [`ColorModel`].
    #[inline]
    pub fn color_model(&self) -> ColorModel {
        self.format.color_model()
    }

    /// Get the value of the element at the given index.
    ///
    /// Return `None` if out-of-bounds.
    #[inline]
    pub fn get_xyc(&self, x: u32, y: u32, channel: u32) -> Option<TensorElement> {
        let w = self.width();
        let h = self.height();

        if w <= x || h <= y {
            return None;
        }

        if let Some(pixel_format) = self.format.pixel_format {
            // NOTE: the name `y` is already taken for the coordinate, so we use `luma` here.
            let [luma, u, v] = pixel_format.decode_yuv_at(&self.buffer, [w, h], [x, y])?;

            match pixel_format.color_model() {
                ColorModel::L => (channel == 0).then_some(TensorElement::U8(luma)),

                // Shouldn't hit BGR and BGRA, but we'll handle it like RGB and RGBA here for completeness.
                ColorModel::RGB | ColorModel::RGBA | ColorModel::BGR | ColorModel::BGRA => {
                    if channel < 3 {
                        let rgb = rgb_from_yuv(
                            luma,
                            u,
                            v,
                            pixel_format.is_limited_yuv_range(),
                            pixel_format.yuv_matrix_coefficients(),
                        );
                        Some(TensorElement::U8(rgb[channel as usize]))
                    } else if channel == 4 {
                        Some(TensorElement::U8(255))
                    } else {
                        None
                    }
                }
            }
        } else {
            let num_channels = self.format.color_model().num_channels();

            re_log::debug_assert!(channel < num_channels as u32);
            if num_channels as u32 <= channel {
                return None;
            }

            let stride = w; // TODO(#6008): support stride
            let offset =
                (y as usize * stride as usize + x as usize) * num_channels + channel as usize;

            match self.format.datatype() {
                ChannelDatatype::U8 => self.buffer.get(offset).copied().map(TensorElement::U8),
                ChannelDatatype::U16 => get(&self.buffer, offset).map(TensorElement::U16),
                ChannelDatatype::U32 => get(&self.buffer, offset).map(TensorElement::U32),
                ChannelDatatype::U64 => get(&self.buffer, offset).map(TensorElement::U64),

                ChannelDatatype::I8 => get(&self.buffer, offset).map(TensorElement::I8),
                ChannelDatatype::I16 => get(&self.buffer, offset).map(TensorElement::I16),
                ChannelDatatype::I32 => get(&self.buffer, offset).map(TensorElement::I32),
                ChannelDatatype::I64 => get(&self.buffer, offset).map(TensorElement::I64),

                ChannelDatatype::F16 => get(&self.buffer, offset).map(TensorElement::F16),
                ChannelDatatype::F32 => get(&self.buffer, offset).map(TensorElement::F32),
                ChannelDatatype::F64 => get(&self.buffer, offset).map(TensorElement::F64),
            }
        }
    }

    /// Cast the buffer to the given type.
    ///
    /// This will never fail.
    /// If the buffer is 5 bytes long and the target type is `f32`, the last byte is just ignored.
    ///
    /// Cheap in most cases, but if the input buffer is not aligned to the element type,
    /// this function will copy the data.
    pub fn to_slice<T: bytemuck::Pod>(&self) -> Cow<'_, [T]> {
        let element_size = std::mem::size_of::<T>();
        let num_elements = self.buffer.len() / element_size;
        let num_bytes = num_elements * element_size;
        let bytes = &self.buffer[..num_bytes];

        if let Ok(slice) = bytemuck::try_cast_slice(bytes) {
            Cow::Borrowed(slice)
        } else {
            // This should happen very rarely.
            // But it can happen, e.g. when logging a `1x1xu8` image followed by a `1x1xf32` image
            // to the same entity path, and they are put in the same chunk.

            re_log::debug_warn_once!(
                "The image buffer was not aligned to the element type {}",
                std::any::type_name::<T>()
            );
            re_tracing::profile_scope!("copy_image_buffer");

            let mut dest = vec![T::zeroed(); num_elements];
            let dest_bytes: &mut [u8] = bytemuck::cast_slice_mut(&mut dest);
            dest_bytes.copy_from_slice(bytes);
            Cow::Owned(dest)
        }
    }

    /// Best-effort.
    ///
    /// `u8` and `u16` images are returned as is.
    ///
    /// Other data types are remapped from the given `data_range`
    /// to the full `u16` range, then rounded.
    ///
    /// Returns `None` for invalid images (if the buffer is the wrong size).
    pub fn to_dynamic_image(&self, data_range: RangeInclusive<f32>) -> Option<image::DynamicImage> {
        re_tracing::profile_function!();

        use image::{DynamicImage, GrayImage, RgbImage, RgbaImage};
        type Gray16Image = image::ImageBuffer<image::Luma<u16>, Vec<u16>>;
        type Rgb16Image = image::ImageBuffer<image::Rgb<u16>, Vec<u16>>;
        type Rgba16Image = image::ImageBuffer<image::Rgba<u16>, Vec<u16>>;

        let (w, h) = (self.width(), self.height());

        if let Some(pixel_format) = self.format.pixel_format {
            // Convert to RGB.
            // TODO(emilk): this can probably be optimized.
            let mut rgb = Vec::with_capacity((w * h * 3) as usize);
            for y in 0..h {
                for x in 0..w {
                    let [r, g, b] = pixel_format.decode_rgb_at(&self.buffer, [w, h], [x, y])?;
                    rgb.push(r);
                    rgb.push(g);
                    rgb.push(b);
                }
            }
            RgbImage::from_vec(w, h, rgb).map(DynamicImage::ImageRgb8)
        } else if self.format.datatype() == ChannelDatatype::U8 {
            let mut u8 = self.buffer.to_vec();
            match self.color_model() {
                ColorModel::L => GrayImage::from_vec(w, h, u8).map(DynamicImage::ImageLuma8),
                ColorModel::RGB => RgbImage::from_vec(w, h, u8).map(DynamicImage::ImageRgb8),
                ColorModel::RGBA => RgbaImage::from_vec(w, h, u8).map(DynamicImage::ImageRgba8),
                ColorModel::BGR => {
                    bgr_to_rgb(&mut u8);
                    RgbImage::from_vec(w, h, u8).map(DynamicImage::ImageRgb8)
                }
                ColorModel::BGRA => {
                    bgra_to_rgba(&mut u8);
                    RgbaImage::from_vec(w, h, u8).map(DynamicImage::ImageRgba8)
                }
            }
        } else if self.format.datatype() == ChannelDatatype::U16 {
            // Lossless conversion of u16, ignoring data_range
            let mut u16 = self.to_slice::<u16>().to_vec();
            match self.color_model() {
                ColorModel::L => Gray16Image::from_vec(w, h, u16).map(DynamicImage::ImageLuma16),
                ColorModel::RGB => Rgb16Image::from_vec(w, h, u16).map(DynamicImage::ImageRgb16),
                ColorModel::RGBA => Rgba16Image::from_vec(w, h, u16).map(DynamicImage::ImageRgba16),
                ColorModel::BGR => {
                    bgr_to_rgb(&mut u16);
                    Rgb16Image::from_vec(w, h, u16).map(DynamicImage::ImageRgb16)
                }
                ColorModel::BGRA => {
                    bgra_to_rgba(&mut u16);
                    Rgba16Image::from_vec(w, h, u16).map(DynamicImage::ImageRgba16)
                }
            }
        } else {
            let mut u16 = self.to_vec_u16(self.format.datatype(), data_range);
            match self.color_model() {
                ColorModel::L => Gray16Image::from_vec(w, h, u16).map(DynamicImage::ImageLuma16),
                ColorModel::RGB => Rgb16Image::from_vec(w, h, u16).map(DynamicImage::ImageRgb16),
                ColorModel::RGBA => Rgba16Image::from_vec(w, h, u16).map(DynamicImage::ImageRgba16),
                ColorModel::BGR => {
                    bgr_to_rgb(&mut u16);
                    Rgb16Image::from_vec(w, h, u16).map(DynamicImage::ImageRgb16)
                }
                ColorModel::BGRA => {
                    bgra_to_rgba(&mut u16);
                    Rgba16Image::from_vec(w, h, u16).map(DynamicImage::ImageRgba16)
                }
            }
        }
    }

    /// See [`Self::to_dynamic_image`].
    pub fn to_rgba8_image(&self, data_range: RangeInclusive<f32>) -> Option<image::RgbaImage> {
        self.to_dynamic_image(data_range).map(|img| img.to_rgba8())
    }

    /// Remaps the given data range to `u16`, with rounding and clamping.
    fn to_vec_u16(&self, datatype: ChannelDatatype, data_range: RangeInclusive<f32>) -> Vec<u16> {
        re_tracing::profile_function!();

        let data_range = emath::Rangef::from(data_range);
        let u16_range = emath::Rangef::new(0.0, u16::MAX as f32);
        let remap_range = |x: f32| -> u16 { emath::remap(x, data_range, u16_range).round() as u16 };

        match datatype {
            ChannelDatatype::U8 => self
                .to_slice::<u8>()
                .iter()
                .map(|&x| remap_range(x as f32))
                .collect(),

            ChannelDatatype::I8 => self
                .to_slice::<i8>()
                .iter()
                .map(|&x| remap_range(x as f32))
                .collect(),

            ChannelDatatype::U16 => self
                .to_slice::<u16>()
                .iter()
                .map(|&x| remap_range(x as f32))
                .collect(),

            ChannelDatatype::I16 => self
                .to_slice::<i16>()
                .iter()
                .map(|&x| remap_range(x as f32))
                .collect(),

            ChannelDatatype::U32 => self
                .to_slice::<u32>()
                .iter()
                .map(|&x| remap_range(x as f32))
                .collect(),

            ChannelDatatype::I32 => self
                .to_slice::<i32>()
                .iter()
                .map(|&x| remap_range(x as f32))
                .collect(),

            ChannelDatatype::U64 => self
                .to_slice::<u64>()
                .iter()
                .map(|&x| remap_range(x as f32))
                .collect(),

            ChannelDatatype::I64 => self
                .to_slice::<i64>()
                .iter()
                .map(|&x| remap_range(x as f32))
                .collect(),

            ChannelDatatype::F16 => self
                .to_slice::<half::f16>()
                .iter()
                .map(|&x| remap_range(x.to_f32()))
                .collect(),

            ChannelDatatype::F32 => self
                .to_slice::<f32>()
                .iter()
                .map(|&x| remap_range(x))
                .collect(),

            ChannelDatatype::F64 => self
                .to_slice::<f64>()
                .iter()
                .map(|&x| remap_range(x as f32))
                .collect(),
        }
    }

    /// Convert this image to an encoded PNG
    pub fn to_png(&self, data_range: RangeInclusive<f32>) -> anyhow::Result<Vec<u8>> {
        if let Some(dynamic_image) = self.to_dynamic_image(data_range) {
            let mut png_bytes = Vec::new();
            if let Err(err) = dynamic_image.write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            ) {
                anyhow::bail!("Failed to encode PNG: {err}");
            }
            Ok(png_bytes)
        } else {
            anyhow::bail!("Invalid image");
        }
    }
}

fn bgr_to_rgb<T: Clone>(bgr_elements: &mut [T]) {
    for bgr in bgr_elements.chunks_exact_mut(3) {
        bgr.swap(0, 2);
    }
}

fn bgra_to_rgba<T: Clone>(bgra_elements: &mut [T]) {
    for bgra in bgra_elements.chunks_exact_mut(4) {
        bgra.swap(0, 2);
    }
}

fn get<T: bytemuck::Pod>(blob: &[u8], element_offset: usize) -> Option<T> {
    // NOTE: `blob` is not necessary aligned to `T`,
    // hence the complexity of this function.

    let size = std::mem::size_of::<T>();
    let byte_offset = element_offset * size;
    if blob.len() <= byte_offset + size {
        return None;
    }

    let slice = &blob[byte_offset..byte_offset + size];

    let mut dest = T::zeroed();
    bytemuck::bytes_of_mut(&mut dest).copy_from_slice(slice);
    Some(dest)
}

#[cfg(test)]
mod tests {
    use re_log_types::hash::Hash64;
    use re_sdk_types::datatypes::ColorModel;
    use re_sdk_types::image::ImageChannelType;

    use super::ImageInfo;
    use crate::image_info::StoredBlobCacheKey;

    fn new_2x2_image_info<T: ImageChannelType>(
        color_model: ColorModel,
        elements: &[T],
    ) -> ImageInfo {
        assert_eq!(elements.len(), 2 * 2);
        ImageInfo {
            buffer_content_hash: StoredBlobCacheKey(Hash64::ZERO), // unused
            buffer: re_sdk_types::datatypes::Blob::from(bytemuck::cast_slice::<_, u8>(elements)),
            format: re_sdk_types::datatypes::ImageFormat::from_color_model(
                [2, 2],
                color_model,
                T::CHANNEL_TYPE,
            ),
            kind: re_sdk_types::image::ImageKind::Color,
        }
    }

    fn dynamic_image_from_png(png_bytes: &[u8]) -> image::DynamicImage {
        image::load_from_memory_with_format(png_bytes, image::ImageFormat::Png).unwrap()
    }

    #[test]
    fn test_image_l_u8_roundtrip() {
        let contents = vec![1_u8, 42_u8, 69_u8, 137_u8];

        let image_info = new_2x2_image_info(ColorModel::L, &contents);
        assert_eq!(image_info.to_slice::<u8>().to_vec(), contents);
        assert_eq!(
            image_info.to_dynamic_image(0.0..=1.0).unwrap(),
            image_info.to_dynamic_image(0.0..=255.0).unwrap(),
            "Data range should be ignored for u8"
        );
        let png_bytes = image_info.to_png(0.0..=255.0).unwrap();
        let dynamic_image = dynamic_image_from_png(&png_bytes);
        if let image::DynamicImage::ImageLuma8(image) = dynamic_image {
            assert_eq!(&image.into_vec(), &contents);
        } else {
            panic!("Expected ImageLuma8, got {dynamic_image:?}");
        }
    }

    #[test]
    fn test_image_l_u16_roundtrip() {
        let contents = vec![1_u16, 42_u16, 69_u16, 137_u16];

        let image_info = new_2x2_image_info(ColorModel::L, &contents);
        assert_eq!(image_info.to_slice::<u16>().to_vec(), contents);
        assert_eq!(
            image_info.to_dynamic_image(0.0..=1.0).unwrap(),
            image_info.to_dynamic_image(0.0..=255.0).unwrap(),
            "Data range should be ignored for u16"
        );
        let png_bytes = image_info.to_png(0.0..=255.0).unwrap();
        let dynamic_image = dynamic_image_from_png(&png_bytes);
        if let image::DynamicImage::ImageLuma16(image) = dynamic_image {
            assert_eq!(&image.into_vec(), &contents);
        } else {
            panic!("Expected ImageLuma8, got {dynamic_image:?}");
        }
    }
}
