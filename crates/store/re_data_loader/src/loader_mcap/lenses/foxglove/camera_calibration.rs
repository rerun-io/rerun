use re_lenses::{Lens, LensError, op};
use re_lenses_core::Selector;
use re_lenses_core::combinators::{MapList, Transform as _};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::Pinhole;

use crate::loader_mcap::lenses::helpers::{
    row_major_3x3_to_column_major, width_height_to_resolution,
};

use super::{FOXGLOVE_TIMESTAMP, IMAGE_PLANE_SUFFIX};

/// Creates a lens for [`foxglove.CameraCalibration`] messages.
///
/// [`foxglove.CameraCalibration`]: https://docs.foxglove.dev/docs/sdk/schemas/camera-calibration
pub fn camera_calibration(time_type: TimeType) -> Result<Lens, LensError> {
    Ok(Lens::for_input_column(
        EntityPathFilter::all(),
        "foxglove.CameraCalibration:message",
    )
    .output_columns(|out| {
        out.time(
            FOXGLOVE_TIMESTAMP,
            time_type,
            Selector::parse(".timestamp")?.then(MapList::new(op::timespec_to_nanos())),
        )?
        .component(
            Pinhole::descriptor_child_frame(),
            Selector::parse(".frame_id")?
                .then(MapList::new(op::string_suffix_nonempty(IMAGE_PLANE_SUFFIX))),
        )?
        .component(
            Pinhole::descriptor_resolution(),
            Selector::parse(".")?.then(MapList::new(width_height_to_resolution())),
        )?
        .component(
            Pinhole::descriptor_image_from_camera(),
            Selector::parse(".K")?.then(MapList::new(row_major_3x3_to_column_major())),
        )?
        .component(
            Pinhole::descriptor_parent_frame(),
            Selector::parse(".frame_id")?,
        )
    })?
    .build())
}
