//! Definitions for the ROS2 `tf2_msgs` package.
//!
//! Based on definitions taken from <https://github.com/ros2/geometry2/tree/rolling/tf2_msgs>

use serde::{Deserialize, Serialize};

use super::geometry_msgs::TransformStamped;

/// A message that contains multiple [`TransformStamped`] messages.
/// This is the primary message type used in ROS2's transform system.
#[derive(Debug, Serialize, Deserialize)]
pub struct TFMessage {
    /// Array of transform stamped messages
    pub transforms: Vec<TransformStamped>,
}
