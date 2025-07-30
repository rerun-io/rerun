//! ROS2 message definitions.
//!
//! This crate provides `serde`-compatible Rust types for a subset of
//! ROS2 message definitions, allowing for deserialization of MCAP files containing
//! ROS2 data into idiomatic Rust structs.
//!
//! The supported message packages include:
//!
//! - [`builtin_interfaces`]: Time and duration representations.
//! - [`std_msgs`]: Common standard messages like [`std_msgs::Header`] and [`std_msgs::ColorRGBA`].

pub mod builtin_interfaces;
pub mod geometry_msgs;
pub mod sensor_msgs;
pub mod std_msgs;
