use super::PixelFormat;

impl PixelFormat {
    /// Do we have an alpha channel?
    #[inline]
    pub fn has_alpha(&self) -> bool {
        match self {
            Self::NV12 | Self::YUY2 => false,
        }
    }
}
