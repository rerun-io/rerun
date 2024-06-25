use super::TensorDimensionSelection;

impl From<u32> for TensorDimensionSelection {
    #[inline]
    fn from(dimension: u32) -> Self {
        Self {
            dimension,
            invert: false,
        }
    }
}
