use std::borrow::Cow;

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
    /// Can be used instead of hashing [`Self::blob`].
    pub blob_row_id: RowId,

    /// The image data, row-wise, with stride=width.
    pub blob: Blob,

    /// Describes the format of [`Self::blob`].
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
            let buf: &[u8] = &self.blob;

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
                ChannelDatatype::U8 => self.blob.get(offset).copied().map(TensorElement::U8),
                ChannelDatatype::U16 => get(&self.blob, offset).map(TensorElement::U16),
                ChannelDatatype::U32 => get(&self.blob, offset).map(TensorElement::U32),
                ChannelDatatype::U64 => get(&self.blob, offset).map(TensorElement::U64),

                ChannelDatatype::I8 => get(&self.blob, offset).map(TensorElement::I8),
                ChannelDatatype::I16 => get(&self.blob, offset).map(TensorElement::I16),
                ChannelDatatype::I32 => get(&self.blob, offset).map(TensorElement::I32),
                ChannelDatatype::I64 => get(&self.blob, offset).map(TensorElement::I64),

                ChannelDatatype::F16 => get(&self.blob, offset).map(TensorElement::F16),
                ChannelDatatype::F32 => get(&self.blob, offset).map(TensorElement::F32),
                ChannelDatatype::F64 => get(&self.blob, offset).map(TensorElement::F64),
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
        let num_elements = self.blob.len() / element_size;
        let num_bytes = num_elements * element_size;
        let bytes = &self.blob[..num_bytes];

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
