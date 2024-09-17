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
