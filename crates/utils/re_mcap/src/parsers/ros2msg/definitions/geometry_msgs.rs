//! Definitions for the ROS2 `geometry_msgs` package.
//!
//! Based on definitions taken from <https://github.com/ros2/common_interfaces/tree/rolling/geometry_msgs>
//!
use serde::{Deserialize, Serialize};

use super::std_msgs::Header;

/// This represents a vector in free space.
///
/// This is semantically different than a point.
/// A vector is always anchored at the origin.
/// When a transform is applied to a vector, only the rotational component is applied.
#[derive(Debug, Serialize, Deserialize)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// This represents an orientation in free space in quaternion form.
#[derive(Debug, Serialize, Deserialize)]
pub struct Quaternion {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

/// This contains the position of a point in free space
#[derive(Debug, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// A representation of pose in free space, composed of position and orientation.
#[derive(Debug, Serialize, Deserialize)]
pub struct Pose {
    pub position: Point,
    pub orientation: Quaternion,
}

/// This represents the transform between two coordinate frames in free space.
#[derive(Debug, Serialize, Deserialize)]
pub struct Transform {
    /// Translation component of the transform
    pub translation: Vector3,

    /// Rotation component of the transform
    pub rotation: Quaternion,
}

/// A transform with a timestamp and frame information.
#[derive(Debug, Serialize, Deserialize)]
pub struct TransformStamped {
    /// Header with timestamp and frame information
    pub header: Header,

    /// The frame id of the child frame
    pub child_frame_id: String,

    /// The actual transform data
    pub transform: Transform,
}
