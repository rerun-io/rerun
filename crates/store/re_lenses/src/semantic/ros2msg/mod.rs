//! Lenses for converting ROS 2 messages to Rerun components and archetypes.

use crate::{Lens, LensBuilderError};

use super::IMAGE_PLANE_SUFFIX;

mod camera_info;
mod fluid_pressure;
mod illuminance;
mod image;
mod log;
mod magnetic_field;
mod occupancy_grid;
mod pose_stamped;
mod relative_humidity;
mod ros_image_helpers;
mod ros_map_helpers;
mod string;
mod temperature;
mod tf_message;
mod voxel_grid;

pub use camera_info::camera_info;
pub use fluid_pressure::fluid_pressure;
pub use illuminance::illuminance;
pub use image::image;
pub use log::log;
pub use magnetic_field::magnetic_field;
pub use occupancy_grid::occupancy_grid;
pub use pose_stamped::pose_stamped;
pub use relative_humidity::relative_humidity;
pub use string::string;
pub use temperature::temperature;
pub use tf_message::tf_message;
pub use voxel_grid::voxel_grid;

/// Builds all ROS 2 message lenses.
pub fn all() -> Result<Vec<Lens>, LensBuilderError> {
    Ok(vec![
        camera_info()?,
        fluid_pressure()?,
        illuminance()?,
        image()?,
        log()?,
        magnetic_field()?,
        occupancy_grid()?,
        pose_stamped()?,
        relative_humidity()?,
        string()?,
        temperature()?,
        tf_message()?,
        voxel_grid()?,
    ])
}
