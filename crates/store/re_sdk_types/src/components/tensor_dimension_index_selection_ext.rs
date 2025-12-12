use super::TensorDimensionIndexSelection;
use crate::datatypes;

impl TensorDimensionIndexSelection {
    /// Creates a new `TensorDimensionIndexSelection` from the given `dimension` and `index`.
    pub fn new(dimension: u32, index: u64) -> Self {
        Self(datatypes::TensorDimensionIndexSelection { dimension, index })
    }
}
