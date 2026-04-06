//! Lenses for converting Foxglove Protobuf messages to Rerun components & archetypes.

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

use re_lenses::{LensError, Lenses, OutputMode};
use re_log_types::TimeType;

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

/// Suffix appended to frame IDs for image planes.
///
/// This is required to match the Rerun model for named pinhole frames, where the image plane has its own frame ID
/// different from the pinhole frame. In ROS/Foxglove, both image and camera info share the same frame ID.
const IMAGE_PLANE_SUFFIX: &str = "_image_plane";

/// Name of the timestamp field in Foxglove messages and name of the corresponding Rerun timeline.
const FOXGLOVE_TIMESTAMP: &str = "timestamp";

/// Creates a collection of all Foxglove lenses.
pub fn foxglove_lenses(time_type: TimeType) -> Result<Lenses, LensError> {
    let mut lenses = Lenses::new(OutputMode::ForwardUnmatched);
    lenses.add_lens(camera_calibration(time_type)?);
    lenses.add_lens(compressed_image(time_type)?);
    lenses.add_lens(compressed_video(time_type)?);
    lenses.add_lens(frame_transform(time_type)?);
    lenses.add_lens(frame_transforms(time_type)?);
    lenses.add_lens(location_fix(time_type)?);
    lenses.add_lens(location_fixes(time_type)?);
    lenses.add_lens(log(time_type)?);
    lenses.add_lens(point_cloud(time_type)?);
    lenses.add_lens(pose_in_frame(time_type)?);
    lenses.add_lens(poses_in_frame(time_type)?);
    lenses.add_lens(raw_image(time_type)?);
    Ok(lenses)
}
