use re_lenses::{Lens, LensError, Op};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::Transform3D;

use crate::loader_mcap::lenses::helpers::{
    list_xyz_struct_to_list_fixed, list_xyzw_struct_to_list_fixed,
};

use super::FOXGLOVE_TIMESTAMP;

/// Creates a lens for [`foxglove.FrameTransform`] messages.
///
/// [`foxglove.FrameTransform`]: https://docs.foxglove.dev/docs/sdk/schemas/frame-transform
pub fn frame_transform() -> Result<Lens, LensError> {
    Ok(
        Lens::for_input_column(EntityPathFilter::all(), "foxglove.FrameTransform:message")
            .output_scatter_columns(|out| {
                out.time(
                    FOXGLOVE_TIMESTAMP,
                    TimeType::TimestampNs,
                    [Op::selector(".timestamp"), Op::time_spec_to_nanos()],
                )
                .component(
                    Transform3D::descriptor_parent_frame(),
                    [Op::selector(".parent_frame_id")],
                )
                .component(
                    Transform3D::descriptor_child_frame(),
                    [Op::selector(".child_frame_id")],
                )
                .component(
                    Transform3D::descriptor_translation(),
                    [
                        Op::selector(".translation"),
                        Op::func(list_xyz_struct_to_list_fixed),
                    ],
                )
                .component(
                    Transform3D::descriptor_quaternion(),
                    [
                        Op::selector(".rotation"),
                        Op::func(list_xyzw_struct_to_list_fixed),
                    ],
                )
            })?
            .build(),
    )
}
