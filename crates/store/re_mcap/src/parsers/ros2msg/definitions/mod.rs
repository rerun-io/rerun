//! ROS2 message definitions.
//!
//! This crate provides `serde`-compatible Rust types for a subset of
//! ROS2 message definitions, allowing for deserialization of MCAP files containing
//! ROS2 data into idiomatic Rust structs.

pub mod builtin_interfaces;
pub mod geometry_msgs;
pub mod rcl_interfaces;
pub mod sensor_msgs;
pub mod std_msgs;
pub mod tf2_msgs;
