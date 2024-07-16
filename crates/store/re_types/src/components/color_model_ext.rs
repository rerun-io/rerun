use super::ColorModel;

impl ColorModel {
    /// 1 for grayscale, 3 for RGB, etc.
    #[inline]
    pub fn num_components(self) -> usize {
        match self {
            Self::L => 1,
            Self::Rgb => 3,
            Self::Rgba => 4,
        }
    }
}
