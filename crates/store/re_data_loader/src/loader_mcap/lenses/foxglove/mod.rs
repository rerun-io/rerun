//! Lenses for converting Foxglove Protobuf messages to Rerun components & archetypes.

mod camera_calibration;
mod compressed_image;
mod compressed_video;
mod frame_transform;
mod frame_transforms;
mod log;
mod pose_in_frame;
mod poses_in_frame;

use re_lenses::{LensError, Lenses, OutputMode};

pub use camera_calibration::camera_calibration;
pub use compressed_image::compressed_image;
pub use compressed_video::compressed_video;
pub use frame_transform::frame_transform;
pub use frame_transforms::frame_transforms;
pub use log::log;
pub use pose_in_frame::pose_in_frame;
pub use poses_in_frame::poses_in_frame;

/// Suffix appended to frame IDs for image planes.
///
/// This is required to match the Rerun model for named pinhole frames, where the image plane has its own frame ID
/// different from the pinhole frame. In ROS/Foxglove, both image and camera info share the same frame ID.
const IMAGE_PLANE_SUFFIX: &str = "_image_plane";

/// Name of the timestamp field in Foxglove messages and name of the corresponding Rerun timeline.
const FOXGLOVE_TIMESTAMP: &str = "timestamp";

/// Creates a collection of all Foxglove lenses.
pub fn foxglove_lenses() -> Result<Lenses, LensError> {
    let mut lenses = Lenses::new(OutputMode::ForwardUnmatched);
    lenses.add_lens(camera_calibration()?);
    lenses.add_lens(compressed_image()?);
    lenses.add_lens(compressed_video()?);
    lenses.add_lens(frame_transform()?);
    lenses.add_lens(frame_transforms()?);
    lenses.add_lens(log()?);
    lenses.add_lens(pose_in_frame()?);
    lenses.add_lens(poses_in_frame()?);
    Ok(lenses)
}
