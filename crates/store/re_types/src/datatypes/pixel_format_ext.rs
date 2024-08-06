use super::PixelFormat;

impl PixelFormat {
    /// Do we have an alpha channel?
    #[inline]
    pub fn has_alpha(&self) -> bool {
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
}
