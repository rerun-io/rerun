use super::TensorDimensionIndexSelection;
use crate::encodings;

impl TensorDimensionIndexSelection {
    /// Creates a new `TensorDimensionIndexSelection` from the given `dimension` and `index`.
    pub fn new(dimension: u32, index: u64) -> Self {
        Self(encodings::TensorDimensionIndexSelection { dimension, index })
    }
}
