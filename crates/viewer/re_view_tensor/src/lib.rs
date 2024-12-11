//! Rerun tensor View.
//!
//! A View dedicated to visualizing tensors with arbitrary dimensionality.

// TODO(#6330): remove unwrap()
#![allow(clippy::unwrap_used)]

mod dimension_mapping;
mod tensor_dimension_mapper;
mod tensor_slice_to_gpu;
mod view_class;
mod visualizer_system;

pub use view_class::TensorView;
