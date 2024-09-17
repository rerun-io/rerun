use crate::image::rgb_from_yuv;

use super::{ChannelDatatype, ColorModel, PixelFormat};

impl PixelFormat {
    /// Do we have an alpha channel?
    #[inline]
    pub fn has_alpha(&self) -> bool {
        match self {
            Self::NV12 | Self::YUY2 => false,
        }
    }

    #[inline]
    /// Is this pixel format floating point?
    pub fn is_float(&self) -> bool {
        match self {
            Self::NV12 | Self::YUY2 => false,
        }
    }

    /// Number of bytes needed to represent an image of the given size.
    #[inline]
    pub fn num_bytes(&self, [w, h]: [u32; 2]) -> usize {
        let num_pixels = w as usize * h as usize;
        match self {
            Self::NV12 => 12 * num_pixels / 8,
            Self::YUY2 => 16 * num_pixels / 8,
        }
    }

    /// The color model derived from this pixel format.
    #[inline]
    pub fn color_model(&self) -> ColorModel {
        match self {
            Self::NV12 | Self::YUY2 => ColorModel::RGB,
        }
    }

    #[inline]
    /// The datatype that this decodes into.
    pub fn datatype(&self) -> ChannelDatatype {
        match self {
            Self::NV12 | Self::YUY2 => ChannelDatatype::U8,
        }
    }

    /// Random-access decoding of a specific pixel of an image.
    ///
    /// Return `None` if out-of-range.
    #[inline]
    pub fn decode_yuv_at(&self, buf: &[u8], [w, h]: [u32; 2], [x, y]: [u32; 2]) -> Option<[u8; 3]> {
        match self {
            Self::NV12 => {
                let uv_offset = w * h;
                let luma = *buf.get((y * w + x) as usize)?;
                let u = *buf.get((uv_offset + (y / 2) * w + x) as usize)?;
                let v = *buf.get((uv_offset + (y / 2) * w + x) as usize + 1)?;
                Some([luma, u, v])
            }

            Self::YUY2 => {
                let index = ((y * w + x) * 2) as usize;
                if x % 2 == 0 {
                    Some([*buf.get(index)?, *buf.get(index + 1)?, *buf.get(index + 3)?])
                } else {
                    Some([*buf.get(index)?, *buf.get(index - 1)?, *buf.get(index + 1)?])
                }
            }
        }
    }

    /// Random-access decoding of a specific pixel of an image.
    ///
    /// Return `None` if out-of-range.
    #[inline]
    pub fn decode_rgb_at(&self, buf: &[u8], [w, h]: [u32; 2], [x, y]: [u32; 2]) -> Option<[u8; 3]> {
        let [y, u, v] = self.decode_yuv_at(buf, [w, h], [x, y])?;
        Some(rgb_from_yuv(y, u, v))
    }
}
