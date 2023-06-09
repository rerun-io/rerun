//! Rerun tensor Space View.
//!
//! A Space View dedicated to visualizing tensors with arbitrary dimensionality.

mod scene_part;
mod space_view_class;
mod tensor_dimension_mapper;
mod tensor_slice_to_gpu;

pub use space_view_class::TensorSpaceView;
