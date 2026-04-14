use re_lenses::{Lens, LensError, op};
use re_lenses_core::Selector;
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::Transform3D;

use super::FOXGLOVE_TIMESTAMP;

/// Creates a lens for [`foxglove.FrameTransform`] messages.
///
/// [`foxglove.FrameTransform`]: https://docs.foxglove.dev/docs/sdk/schemas/frame-transform
pub fn frame_transform(time_type: TimeType) -> Result<Lens, LensError> {
    Ok(
        Lens::for_input_column(EntityPathFilter::all(), "foxglove.FrameTransform:message")
            .output_scatter_columns(|out| {
                out.time(
                    FOXGLOVE_TIMESTAMP,
                    time_type,
                    Selector::parse(".timestamp")?.pipe(op::timespec_to_nanos()),
                )?
                .component(
                    Transform3D::descriptor_parent_frame(),
                    Selector::parse(".parent_frame_id")?,
                )?
                .component(
                    Transform3D::descriptor_child_frame(),
                    Selector::parse(".child_frame_id")?,
                )?
                .component(
                    Transform3D::descriptor_translation(),
                    Selector::parse(".translation")?
                        .pipe(op::struct_to_fixed_size_list_f32(["x", "y", "z"])),
                )?
                .component(
                    Transform3D::descriptor_quaternion(),
                    Selector::parse(".rotation")?
                        .pipe(op::struct_to_fixed_size_list_f32(["x", "y", "z", "w"])),
                )
            })?
            .build(),
    )
}
