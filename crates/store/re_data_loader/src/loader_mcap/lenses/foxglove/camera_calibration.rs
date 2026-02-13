use re_lenses::{Lens, LensError, Op};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::Pinhole;

use crate::loader_mcap::lenses::helpers::{
    list_3x3_row_major_to_column_major, width_height_to_resolution,
};

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
            [Op::selector(".timestamp"), Op::time_spec_to_nanos()],
        )
        .component(
            Pinhole::descriptor_child_frame(),
            [
                Op::selector(".frame_id"),
                Op::string_suffix_nonempty(IMAGE_PLANE_SUFFIX),
            ],
        )
        .component(
            Pinhole::descriptor_resolution(),
            [Op::func(width_height_to_resolution)],
        )
        .component(
            Pinhole::descriptor_image_from_camera(),
            [
                Op::selector(".K"),
                Op::func(list_3x3_row_major_to_column_major),
            ],
        )
        .component(
            Pinhole::descriptor_parent_frame(),
            [Op::selector(".frame_id")],
        )
    })?
    .build())
}
