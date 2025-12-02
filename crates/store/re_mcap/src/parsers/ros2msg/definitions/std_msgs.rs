//! Definitions for the ROS2 `std_msgs` package.
//!
//! Based on definitions taken from <https://github.com/ros2/common_interfaces/tree/rolling/std_msgs>

use serde::{Deserialize, Serialize};

use super::builtin_interfaces::Time;

/// Color representation in RGBA format
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ColorRGBA {
    /// Red channel value (0.0-1.0)
    pub r: f32,

    /// Green channel value (0.0-1.0)
    pub g: f32,

    /// Blue channel value (0.0-1.0)
    pub b: f32,

    /// Alpha channel value (0.0-1.0)
    pub a: f32,
}

/// A string type used in ROS2 messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StringMessage {
    /// The string data.
    pub data: String,
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
