use re_lenses::{Lens, LensError, Op};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::Transform3D;

use crate::loader_mcap::lenses::helpers::{
    list_xyz_struct_to_list_fixed, list_xyzw_struct_to_list_fixed,
};

use super::FOXGLOVE_TIMESTAMP;

/// Creates a lens for [`foxglove.FrameTransforms`] messages.
///
/// [`foxglove.FrameTransforms`]: https://docs.foxglove.dev/docs/sdk/schemas/frame-transforms
pub fn frame_transforms() -> Result<Lens, LensError> {
    Ok(
        Lens::for_input_column(EntityPathFilter::all(), "foxglove.FrameTransforms:message")
            .output_scatter_columns(|out| {
                out.time(
                    FOXGLOVE_TIMESTAMP,
                    TimeType::TimestampNs,
                    [
                        Op::selector(".transforms[].timestamp"),
                        Op::time_spec_to_nanos(),
                    ],
                )
                .component(
                    Transform3D::descriptor_parent_frame(),
                    [Op::selector(".transforms[].parent_frame_id")],
                )
                .component(
                    Transform3D::descriptor_child_frame(),
                    [Op::selector(".transforms[].child_frame_id")],
                )
                .component(
                    Transform3D::descriptor_translation(),
                    [
                        Op::selector(".transforms[].translation"),
                        Op::func(list_xyz_struct_to_list_fixed),
                    ],
                )
                .component(
                    Transform3D::descriptor_quaternion(),
                    [
                        Op::selector(".transforms[].rotation"),
                        Op::func(list_xyzw_struct_to_list_fixed),
                    ],
                )
            })?
            .build(),
    )
}
