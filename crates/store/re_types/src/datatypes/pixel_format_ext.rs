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

    /// Number of bits needed to represent a single pixel.
    ///
    /// Note that this is not necessarily divisible by 8!
    #[inline]
    pub fn bits_per_pixel(&self) -> usize {
        match self {
            Self::NV12 => 12,
            Self::YUY2 => 16,
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
}
