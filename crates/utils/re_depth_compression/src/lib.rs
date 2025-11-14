//! Utilities for decoding depth compression formats.

pub mod ros_rvl;

pub use ros_rvl::{
    RvlDecodeError, RvlMetadata, decode_ros_rvl_f32, decode_ros_rvl_u16, parse_ros_rvl_metadata,
};
