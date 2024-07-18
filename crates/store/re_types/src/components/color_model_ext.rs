use super::ColorModel;

impl ColorModel {
    /// 1 for grayscale, 3 for RGB, etc.
    #[doc(alias = "components")]
    #[doc(alias = "depth")]
    #[inline]
    pub fn num_channels(self) -> usize {
        match self {
            Self::L => 1,
            Self::Rgb => 3,
            Self::Rgba => 4,
        }
    }

    /// Do we have an alpha channel?
    #[inline]
    pub fn has_alpha(&self) -> bool {
        match self {
            Self::L | Self::Rgb => false,
            Self::Rgba => true,
        }
    }
}
