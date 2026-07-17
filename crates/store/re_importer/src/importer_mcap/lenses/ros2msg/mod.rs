//! Lenses for converting ROS 2 messages to Rerun components & archetypes.

mod camera_info;
mod log;
mod magnetic_field;
mod occupancy_grid;
mod pose_stamped;
mod ros_map_helpers;
mod string;
mod voxel_grid;

use re_lenses::{LensBuilderError, Lenses, OutputMode};
use re_log_types::TimeType;

use super::IMAGE_PLANE_SUFFIX;

pub use camera_info::camera_info;
pub use log::log;
pub use magnetic_field::magnetic_field;
pub use occupancy_grid::occupancy_grid;
pub use pose_stamped::pose_stamped;
pub use string::string;
pub use voxel_grid::voxel_grid;

/// Name of the header-derived ROS 2 timeline.
const ROS2_TIMESTAMP: &str = "ros2_timestamp";

/// Adds all ROS 2 message lenses to an existing collection.
pub fn add_ros2msg_lenses(
    lenses: &mut Lenses,
    time_type: TimeType,
) -> Result<(), LensBuilderError> {
    *lenses = std::mem::replace(lenses, Lenses::new(OutputMode::ForwardUnmatched))
        .add_lens(camera_info(time_type)?)
        .add_lens(log(time_type)?)
        .add_lens(magnetic_field(time_type)?)
        .add_lens(occupancy_grid(time_type)?)
        .add_lens(pose_stamped(time_type)?)
        .add_lens(string(time_type)?)
        .add_lens(voxel_grid(time_type)?);
    Ok(())
}
