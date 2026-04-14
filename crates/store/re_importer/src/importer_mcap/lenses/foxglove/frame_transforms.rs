use re_lenses::{Lens, LensError, op};
use re_lenses_core::Selector;
use re_log_types::TimeType;
use re_sdk_types::archetypes::Transform3D;

use super::FOXGLOVE_TIMESTAMP;

/// Creates a lens for [`foxglove.FrameTransforms`] messages.
///
/// [`foxglove.FrameTransforms`]: https://docs.foxglove.dev/docs/sdk/schemas/frame-transforms
pub fn frame_transforms(time_type: TimeType) -> Result<Lens, LensError> {
    Ok(Lens::for_input_column("foxglove.FrameTransforms:message")
        .output_scatter_columns(|out| {
            out.time(
                FOXGLOVE_TIMESTAMP,
                time_type,
                Selector::parse(".transforms[].timestamp")?.pipe(op::timespec_to_nanos()),
            )?
            .component(
                Transform3D::descriptor_parent_frame(),
                Selector::parse(".transforms[].parent_frame_id")?,
            )?
            .component(
                Transform3D::descriptor_child_frame(),
                Selector::parse(".transforms[].child_frame_id")?,
            )?
            .component(
                Transform3D::descriptor_translation(),
                Selector::parse(".transforms[].translation")?
                    .pipe(op::struct_to_fixed_size_list_f32(["x", "y", "z"])),
            )?
            .component(
                Transform3D::descriptor_quaternion(),
                Selector::parse(".transforms[].rotation")?
                    .pipe(op::struct_to_fixed_size_list_f32(["x", "y", "z", "w"])),
            )
        })?
        .build())
}
