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

use super::IMAGE_PLANE_SUFFIX;

pub use camera_info::camera_info;
pub use log::log;
pub use magnetic_field::magnetic_field;
pub use occupancy_grid::occupancy_grid;
pub use pose_stamped::pose_stamped;
pub use string::string;
pub use voxel_grid::voxel_grid;

/// Adds all ROS 2 message lenses to an existing collection.
///
/// The `ros2_timestamp` timeline comes from the reflection decoder's generic stamp handling.
pub fn add_ros2msg_lenses(lenses: &mut Lenses) -> Result<(), LensBuilderError> {
    *lenses = std::mem::replace(lenses, Lenses::new(OutputMode::ForwardUnmatched))
        .add_lens(camera_info()?)
        .add_lens(log()?)
        .add_lens(magnetic_field()?)
        .add_lens(occupancy_grid()?)
        .add_lens(pose_stamped()?)
        .add_lens(string()?)
        .add_lens(voxel_grid()?);
    Ok(())
}
