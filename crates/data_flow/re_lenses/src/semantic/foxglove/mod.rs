//! Lenses for converting Foxglove Protobuf messages to Rerun components and archetypes.

use re_log_types::TimeType;

use crate::{Lens, LensBuilderError};

use super::IMAGE_PLANE_SUFFIX;

mod camera_calibration;
mod compressed_image;
mod compressed_video;
mod frame_transform;
mod frame_transforms;
mod location_fix;
mod location_fixes;
mod log;
mod packed_element_field;
mod point_cloud;
mod pose_in_frame;
mod poses_in_frame;
mod raw_image;
mod voxel_grid;

pub use camera_calibration::camera_calibration;
pub use compressed_image::compressed_image;
pub use compressed_video::compressed_video;
pub use frame_transform::frame_transform;
pub use frame_transforms::frame_transforms;
pub use location_fix::location_fix;
pub use location_fixes::location_fixes;
pub use log::log;
pub use point_cloud::point_cloud;
pub use pose_in_frame::pose_in_frame;
pub use poses_in_frame::poses_in_frame;
pub use raw_image::raw_image;
pub use voxel_grid::voxel_grid;

/// Name of the timestamp field in Foxglove messages and name of the corresponding Rerun timeline.
const FOXGLOVE_TIMESTAMP: &str = "timestamp";

/// Builds all Foxglove lenses with the specified time type.
pub fn all(time_type: TimeType) -> Result<Vec<Lens>, LensBuilderError> {
    Ok(vec![
        camera_calibration(time_type)?,
        compressed_image(time_type)?,
        compressed_video(time_type)?,
        frame_transform(time_type)?,
        frame_transforms(time_type)?,
        location_fix(time_type)?,
        location_fixes(time_type)?,
        log(time_type)?,
        point_cloud(time_type)?,
        pose_in_frame(time_type)?,
        poses_in_frame(time_type)?,
        raw_image(time_type)?,
        voxel_grid(time_type)?,
    ])
}
