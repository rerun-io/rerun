use re_lenses::{Lens, LensError, op};
use re_lenses_core::Selector;
use re_log_types::TimeType;
use re_sdk_types::archetypes::Pinhole;

use crate::importer_mcap::lenses::helpers::row_major_3x3_to_column_major;

use super::{FOXGLOVE_TIMESTAMP, IMAGE_PLANE_SUFFIX};

/// Creates a lens for [`foxglove.CameraCalibration`] messages.
///
/// [`foxglove.CameraCalibration`]: https://docs.foxglove.dev/docs/sdk/schemas/camera-calibration
pub fn camera_calibration(time_type: TimeType) -> Result<Lens, LensError> {
    Ok(Lens::for_input_column("foxglove.CameraCalibration:message")
        .output_columns(|out| {
            out.time(
                FOXGLOVE_TIMESTAMP,
                time_type,
                Selector::parse(".timestamp")?.pipe(op::timespec_to_nanos()),
            )?
            .component(
                Pinhole::descriptor_child_frame(),
                Selector::parse(".frame_id")?.pipe(op::string_suffix_nonempty(IMAGE_PLANE_SUFFIX)),
            )?
            .component(
                Pinhole::descriptor_resolution(),
                Selector::parse(".")?.pipe(op::struct_to_fixed_size_list_f32(["width", "height"])),
            )?
            .component(
                Pinhole::descriptor_image_from_camera(),
                Selector::parse(".K")?.pipe(row_major_3x3_to_column_major()),
            )?
            .component(
                Pinhole::descriptor_parent_frame(),
                Selector::parse(".frame_id")?,
            )
        })?
        .build())
}
