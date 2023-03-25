//! Helper used to work with `re_log_types::Tensor` as an `ndarray`
//!
//! The actual conversion into / out of `ndarray` is now done using `TryFrom` trait and
//! and implemented in `re_log_types/src/component_types/tensor.rs`

pub mod dimension_mapping;
