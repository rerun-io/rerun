//! Definitions for the ROS2 `tf2_msgs` package.
//!
//! Based on definitions taken from <https://github.com/ros2/geometry2/tree/rolling/tf2_msgs>
//!

use super::geometry_msgs::TransformStamped;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TFMessage {
    pub transforms: Vec<TransformStamped>,
}
