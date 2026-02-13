use super::AxisLength;

impl Default for AxisLength {
    #[inline]
    fn default() -> Self {
        1.0.into()
    }
}

impl From<AxisLength> for f32 {
    #[inline]
    fn from(val: AxisLength) -> Self {
        val.0.into()
    }
}
