//! Rerun tensor view.
//!
//! A view dedicated to visualizing tensors with arbitrary dimensionality.

// TODO(#6330): remove unwrap()
#![allow(clippy::unwrap_used)]

mod dimension_mapping;
mod tensor_dimension_mapper;
mod tensor_slice_to_gpu;
mod view_class;
mod visualizer_system;

pub use view_class::TensorView;

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

    #[allow(dead_code)] // Used for tests
    pub fn unnamed(size: u64) -> Self {
        Self { size, name: None }
    }

    #[allow(dead_code)] // Used for tests
    pub fn named(size: u64, name: impl Into<re_types::ArrowString>) -> Self {
        Self {
            size,
            name: Some(name.into()),
        }
    }
}
