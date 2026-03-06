use re_arrow_combinators::{Selector, Transform as _, map::MapList};
use re_lenses::{Lens, LensError, op};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::{CoordinateFrame, Pinhole};

use crate::loader_mcap::lenses::helpers::row_major_3x3_to_column_major;

use super::{FOXGLOVE_TIMESTAMP, IMAGE_PLANE_SUFFIX};

/// Creates a lens for [`foxglove.CameraCalibration`] messages.
///
/// [`foxglove.CameraCalibration`]: https://docs.foxglove.dev/docs/sdk/schemas/camera-calibration
pub fn camera_calibration() -> Result<Lens, LensError> {
    Ok(Lens::for_input_column(
        EntityPathFilter::all(),
        "foxglove.CameraCalibration:message",
    )
    .output_columns(|out| {
        out.time(
            FOXGLOVE_TIMESTAMP,
            TimeType::TimestampNs,
            Selector::parse(".timestamp")?.then(MapList::new(op::timespec_to_nanos())),
        )?
        .component(
            CoordinateFrame::descriptor_frame(),
            Selector::parse(".frame_id")?
                .then(MapList::new(op::string_suffix_nonempty(IMAGE_PLANE_SUFFIX))),
        )?
        .component(
            Pinhole::descriptor_image_from_camera(),
            Selector::parse(".K")?.then(MapList::new(row_major_3x3_to_column_major())),
        )?
        .component(
            CoordinateFrame::descriptor_frame(),
            Selector::parse(".frame_id")?,
        )
    })?
    .build())
}
