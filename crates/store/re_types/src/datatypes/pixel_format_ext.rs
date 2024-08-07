use super::{ColorModel, PixelFormat};

impl PixelFormat {
    /// Do we have an alpha channel?
    #[inline]
    pub fn has_alpha(&self) -> Option<bool> {
        match self {
            Self::NV12 | Self::YUY2 => Some(false),
            Self::GENERIC => None,
        }
    }

    #[inline]
    /// Is this pixel format floating point?
    pub fn is_float(&self) -> Option<bool> {
        match self {
            Self::NV12 | Self::YUY2 => Some(false),
            Self::GENERIC => None,
        }
    }

    /// Number of bits needed to represent a single pixel.
    ///
    /// Note that this is not necessarily divisible by 8!
    #[inline]
    pub fn bits_per_pixel(&self) -> Option<usize> {
        match self {
            Self::NV12 => Some(12),
            Self::YUY2 => Some(16),
            Self::GENERIC => None,
        }
    }

    /// The color model derived from this pixel format.
    #[inline]
    pub fn color_model(&self) -> Option<ColorModel> {
        match self {
            Self::NV12 | Self::YUY2 => Some(ColorModel::RGB),
            Self::GENERIC => None,
        }
    }
}
