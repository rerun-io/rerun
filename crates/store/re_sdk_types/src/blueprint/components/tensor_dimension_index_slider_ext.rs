use super::TensorDimensionIndexSlider;
use crate::blueprint::encodings;

impl TensorDimensionIndexSlider {
    /// Creates a new `TensorDimensionIndexSlider` to determine the index for a given `dimension`.
    pub fn new(dimension: u32) -> Self {
        Self(encodings::TensorDimensionIndexSlider { dimension })
    }
}
