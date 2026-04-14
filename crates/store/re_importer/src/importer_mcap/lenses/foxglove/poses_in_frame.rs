use re_lenses::{Lens, LensError, op};
use re_lenses_core::Selector;
use re_log_types::TimeType;
use re_sdk_types::archetypes::{CoordinateFrame, InstancePoses3D};

use super::FOXGLOVE_TIMESTAMP;

/// Creates a lens for [`foxglove.PosesInFrame`] messages.
///
/// [`foxglove.PosesInFrame`]: https://docs.foxglove.dev/docs/sdk/schemas/poses-in-frame
pub fn poses_in_frame(time_type: TimeType) -> Result<Lens, LensError> {
    Ok(Lens::for_input_column("foxglove.PosesInFrame:message")
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
                Selector::parse(".poses[].position")?
                    .pipe(op::struct_to_fixed_size_list_f32(["x", "y", "z"])),
            )?
            .component(
                InstancePoses3D::descriptor_quaternions(),
                Selector::parse(".poses[].orientation")?
                    .pipe(op::struct_to_fixed_size_list_f32(["x", "y", "z", "w"])),
            )
        })?
        .build())
}
