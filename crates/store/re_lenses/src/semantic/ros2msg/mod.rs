//! Lenses for converting ROS 2 messages to Rerun components and archetypes.

use crate::{Lens, LensBuilderError};

use super::IMAGE_PLANE_SUFFIX;

mod camera_info;
mod log;
mod magnetic_field;
mod occupancy_grid;
mod pose_stamped;
mod ros_map_helpers;
mod string;
mod voxel_grid;

pub use camera_info::camera_info;
pub use log::log;
pub use magnetic_field::magnetic_field;
pub use occupancy_grid::occupancy_grid;
pub use pose_stamped::pose_stamped;
pub use string::string;
pub use voxel_grid::voxel_grid;

/// Builds all ROS 2 message lenses.
pub fn all() -> Result<Vec<Lens>, LensBuilderError> {
    Ok(vec![
        camera_info()?,
        log()?,
        magnetic_field()?,
        occupancy_grid()?,
        pose_stamped()?,
        string()?,
        voxel_grid()?,
    ])
}
