use crate::datatypes;

use super::TensorDimensionIndexSelection;

impl TensorDimensionIndexSelection {
    /// Creates a new `TensorDimensionIndexSelection` from the given `dimension` and `index`.
    pub fn new(dimension: u32, index: u64) -> Self {
        Self(datatypes::TensorDimensionIndexSelection { dimension, index })
    }
}
