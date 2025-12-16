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

// A Pose with reference coordinate frame and timestamp.
#[derive(Debug, Serialize, Deserialize)]
pub struct PoseStamped {
    pub header: Header,
    pub pose: Pose,
}

/// This represents the transform between two coordinate frames in free space.
#[derive(Debug, Serialize, Deserialize)]
pub struct Transform {
    pub translation: Vector3,
    pub rotation: Quaternion,
}

/// This expresses a transform from coordinate frame `header.frame_id`
/// to the coordinate frame `child_frame_id` at the time of `header.stamp`
///
/// This message is mostly used by the [`tf2`](https://docs.ros.org/en/rolling/p/tf2/) package.
/// See its documentation for more information.
///
/// The `child_frame_id` is necessary in addition to the `frame_id`
/// in the Header to communicate the full reference for the transform
/// in a self contained message.
#[derive(Debug, Serialize, Deserialize)]
pub struct TransformStamped {
    pub header: Header,
    pub child_frame_id: String,
    pub transform: Transform,
}
