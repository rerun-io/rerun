use std::{borrow::Cow, ops::RangeInclusive};

use re_chunk::RowId;
use re_types::{
    components::Colormap,
    datatypes::{Blob, ChannelDatatype, ColorModel, ImageFormat, PixelFormat},
    image::ImageKind,
    tensor_data::TensorElement,
};

/// Represents an `Image`, `SegmentationImage` or `DepthImage`.
///
/// It has enough information to render the image on the screen.
#[derive(Clone)]
pub struct ImageInfo {
    /// The row id that contaoned the blob.
    ///
    /// Can be used instead of hashing [`Self::buffer`].
    pub buffer_row_id: RowId,

    /// The image data, row-wise, with stride=width.
    pub buffer: Blob,

    /// Describes the format of [`Self::buffer`].
    pub format: ImageFormat,

    /// Color, Depth, or Segmentation?
    pub kind: ImageKind,

    /// Primarily for depth images atm
    pub colormap: Option<Colormap>,
    // TODO(#6386): `PixelFormat` and `ColorModel`
}

impl ImageInfo {
    #[inline]
    pub fn width(&self) -> u32 {
        self.format.width
    }

    #[inline]
    pub fn height(&self) -> u32 {
        self.format.height
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
            let buf: &[u8] = &self.buffer;

            // NOTE: the name `y` is already taken for the coordinate, so we use `luma` here.
            let [luma, u, v] = match pixel_format {
                PixelFormat::NV12 => {
                    let uv_offset = w * h;
                    let luma = buf[(y * w + x) as usize];
                    let u = buf[(uv_offset + (y / 2) * w + x) as usize];
                    let v = buf[(uv_offset + (y / 2) * w + x) as usize + 1];
                    [luma, u, v]
                }

                PixelFormat::YUY2 => {
                    let index = ((y * w + x) * 2) as usize;
                    if x % 2 == 0 {
                        [buf[index], buf[index + 1], buf[index + 3]]
                    } else {
                        [buf[index], buf[index - 1], buf[index + 1]]
                    }
                }
            };

            match pixel_format.color_model() {
                ColorModel::L => (channel == 0).then_some(TensorElement::U8(luma)),

                ColorModel::RGB | ColorModel::RGBA => {
                    if channel < 3 {
                        let rgb = rgb_from_yuv(luma, u, v);
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

            debug_assert!(channel < num_channels as u32);
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

            if cfg!(debug_asserttions) {
                re_log::warn_once!(
                    "The image buffer was not aligned to the element type {}",
                    std::any::type_name::<T>()
                );
            }
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
    pub fn to_dynamic_image(
        &self,
        data_range: &RangeInclusive<f32>,
    ) -> Option<image::DynamicImage> {
        re_tracing::profile_function!();

        use image::{DynamicImage, GrayImage, RgbImage, RgbaImage};
        type Gray16Image = image::ImageBuffer<image::Luma<u16>, Vec<u16>>;
        type Rgb16Image = image::ImageBuffer<image::Rgb<u16>, Vec<u16>>;
        type Rgba16Image = image::ImageBuffer<image::Rgba<u16>, Vec<u16>>;

        let (w, h) = (self.width(), self.height());

        if let Some(pixel_format) = self.format.pixel_format {
            // Convert to RGB:
            let buf: &[u8] = &self.buffer;
            let mut rgb = Vec::with_capacity((w * h * 3) as usize);
            for y in 0..h {
                for x in 0..w {
                    // NOTE: the name `y` is already taken for the coordinate, so we use `luma` here.
                    let [luma, u, v] = match pixel_format {
                        PixelFormat::NV12 => {
                            let uv_offset = w * h;
                            let luma = buf[(y * w + x) as usize];
                            let u = buf[(uv_offset + (y / 2) * w + x) as usize];
                            let v = buf[(uv_offset + (y / 2) * w + x) as usize + 1];
                            [luma, u, v]
                        }

                        PixelFormat::YUY2 => {
                            let index = ((y * w + x) * 2) as usize;
                            if x % 2 == 0 {
                                [buf[index], buf[index + 1], buf[index + 3]]
                            } else {
                                [buf[index], buf[index - 1], buf[index + 1]]
                            }
                        }
                    };
                    let [r, g, b] = rgb_from_yuv(luma, u, v);
                    rgb.push(r);
                    rgb.push(g);
                    rgb.push(b);
                }
            }
            RgbImage::from_vec(w, h, rgb).map(DynamicImage::ImageRgb8)
        } else if self.format.datatype() == ChannelDatatype::U8 {
            let u8 = self.buffer.to_vec();
            match self.color_model() {
                ColorModel::L => GrayImage::from_vec(w, h, u8).map(DynamicImage::ImageLuma8),
                ColorModel::RGB => RgbImage::from_vec(w, h, u8).map(DynamicImage::ImageRgb8),
                ColorModel::RGBA => RgbaImage::from_vec(w, h, u8).map(DynamicImage::ImageRgba8),
            }
        } else if self.format.datatype() == ChannelDatatype::U16 {
            // Lossless conversion of u16, ignoring data_range
            let u16 = self.to_slice::<u16>().to_vec();
            match self.color_model() {
                ColorModel::L => Gray16Image::from_vec(w, h, u16).map(DynamicImage::ImageLuma16),
                ColorModel::RGB => Rgb16Image::from_vec(w, h, u16).map(DynamicImage::ImageRgb16),
                ColorModel::RGBA => Rgba16Image::from_vec(w, h, u16).map(DynamicImage::ImageRgba16),
            }
        } else {
            let u16 = self.to_vec_u16(self.format.datatype(), data_range);
            match self.color_model() {
                ColorModel::L => Gray16Image::from_vec(w, h, u16).map(DynamicImage::ImageLuma16),
                ColorModel::RGB => Rgb16Image::from_vec(w, h, u16).map(DynamicImage::ImageRgb16),
                ColorModel::RGBA => Rgba16Image::from_vec(w, h, u16).map(DynamicImage::ImageRgba16),
            }
        }
    }

    /// Remaps the given data range to `u16`, with rounding and clamping.
    fn to_vec_u16(&self, datatype: ChannelDatatype, data_range: &RangeInclusive<f32>) -> Vec<u16> {
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

/// Sets the color standard for the given YUV color.
///
/// This conversion mirrors the function of the same name in `crates/viewer/re_renderer/shader/decodings.wgsl`
///
/// Specifying the color standard should be exposed in the future [#3541](https://github.com/rerun-io/rerun/pull/3541)
fn rgb_from_yuv(y: u8, u: u8, v: u8) -> [u8; 3] {
    let (y, u, v) = (y as f32, u as f32, v as f32);

    // rescale YUV values
    let y = (y - 16.0) / 219.0;
    let u = (u - 128.0) / 224.0;
    let v = (v - 128.0) / 224.0;

    // BT.601 (aka. SDTV, aka. Rec.601). wiki: https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.601_conversion
    let r = y + 1.402 * v;
    let g = y - 0.344 * u - 0.714 * v;
    let b = y + 1.772 * u;

    // BT.709 (aka. HDTV, aka. Rec.709). wiki: https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.709_conversion
    // let r = y + 1.575 * v;
    // let g = y - 0.187 * u - 0.468 * v;
    // let b = y + 1.856 * u;

    [(255.0 * r) as u8, (255.0 * g) as u8, (255.0 * b) as u8]
}
