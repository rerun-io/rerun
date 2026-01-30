//! Definitions for the ROS2 `std_msgs` package.
//!
//! Based on definitions taken from <https://github.com/ros2/common_interfaces/tree/rolling/std_msgs>

use serde::{Deserialize, Serialize};

use super::builtin_interfaces::Time;

/// A string type used in ROS2 messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StringMessage {
    /// The string data.
    pub data: String,
}

/// An array of Float64 values used in ROS2 messages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Float64ArrayMessage {
    /// The array of Float64 data.
    pub data: Vec<f64>,
}

/// MultiArrayDimension specifies one dimension of a multi-dimensional array.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiArrayDimension {
    /// Label of the dimension, e.g. "height" or "width".
    pub label: String,
    /// Size of the dimension.
    pub size: u32,
    /// Stride of the dimension.
    pub stride: u32,
}

/// MultiArrayLayout specifies the layout of a multi-dimensional array.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiArrayLayout {
    /// Array of dimension properties.
    pub dim: Vec<MultiArrayDimension>,
    /// Padding data at the beginning of the data array.
    pub data_offset: u32,
}

/// A multi-dimensional array of Float64 values used in ROS2 messages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Float64MultiArrayMessage {
    /// Specification of data layout.
    pub layout: MultiArrayLayout,
    /// Array of Float64 data.
    pub data: Vec<f64>,
}

/// Standard metadata for higher-level stamped data types.
///
/// This is generally used to communicate timestamped data
/// in a particular coordinate frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    /// Two-integer timestamp that is expressed as seconds and nanoseconds.
    pub stamp: Time,

    /// Transform frame with which this data is associated.
    pub frame_id: String,
}
