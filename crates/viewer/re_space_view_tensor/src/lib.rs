//! Rerun tensor Space View.
//!
//! A Space View dedicated to visualizing tensors with arbitrary dimensionality.

// TODO(#6330): remove unwrap()
#![allow(clippy::unwrap_used)]

mod dimension_mapping;
mod space_view_class;
mod tensor_dimension_mapper;
mod tensor_slice_to_gpu;
mod visualizer_system;

pub use space_view_class::TensorSpaceView;

/// Information about a dimension of a tensor.
struct TensorDimension {
    pub size: u64,
    pub name: Option<re_types::ArrowString>,
}

impl TensorDimension {
    pub fn from_tensor_data(tensor_data: &re_types::datatypes::TensorData) -> Vec<Self> {
        tensor_data
            .shape
            .iter()
            .enumerate()
            .map(|(dim_idx, dim_len)| Self {
                size: *dim_len,
                name: tensor_data.dim_name(dim_idx).cloned(),
            })
            .collect()
    }
}
