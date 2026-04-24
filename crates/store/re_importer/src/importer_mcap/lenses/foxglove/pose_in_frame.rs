use re_lenses::{Lens, LensBuilderError, op};
use re_lenses_core::Selector;
use re_log_types::TimeType;
use re_sdk_types::archetypes::{CoordinateFrame, InstancePoses3D};

use super::FOXGLOVE_TIMESTAMP;

/// Creates a lens for [`foxglove.PoseInFrame`] messages.
///
/// [`foxglove.PoseInFrame`]: https://docs.foxglove.dev/docs/sdk/schemas/pose-in-frame
pub fn pose_in_frame(time_type: TimeType) -> Result<Lens, LensBuilderError> {
    Ok(Lens::for_input_column("foxglove.PoseInFrame:message")
        .output_columns(|out| {
            out.time(
                FOXGLOVE_TIMESTAMP,
                time_type,
                Selector::parse(".timestamp")?.pipe(op::timespec_to_nanos()),
            )?
            .component(
                CoordinateFrame::descriptor_frame(),
                Selector::parse(".frame_id")?,
            )?
            .component(
                InstancePoses3D::descriptor_translations(),
                Selector::parse(".pose.position")?
                    .pipe(op::struct_to_fixed_size_list_f32(["x", "y", "z"])),
            )?
            .component(
                InstancePoses3D::descriptor_quaternions(),
                Selector::parse(".pose.orientation")?
                    .pipe(op::struct_to_fixed_size_list_f32(["x", "y", "z", "w"])),
            )
        })?
        .build())
}
