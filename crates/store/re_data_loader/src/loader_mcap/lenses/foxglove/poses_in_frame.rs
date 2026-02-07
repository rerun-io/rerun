use re_lenses::{Lens, LensError, Op};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::{CoordinateFrame, InstancePoses3D};

use crate::loader_mcap::lenses::helpers::{
    list_xyz_struct_to_list_fixed, list_xyzw_struct_to_list_fixed,
};

use super::FOXGLOVE_TIMESTAMP;

/// Creates a lens for [`foxglove.PosesInFrame`] messages.
///
/// [`foxglove.PosesInFrame`]: https://docs.foxglove.dev/docs/sdk/schemas/poses-in-frame
pub fn poses_in_frame() -> Result<Lens, LensError> {
    Ok(
        Lens::for_input_column(EntityPathFilter::all(), "foxglove.PosesInFrame:message")
            .output_columns(|out| {
                out.time(
                    FOXGLOVE_TIMESTAMP,
                    TimeType::TimestampNs,
                    [Op::selector(".timestamp"), Op::time_spec_to_nanos()],
                )
                .component(
                    CoordinateFrame::descriptor_frame(),
                    [Op::selector(".frame_id")],
                )
                .component(
                    InstancePoses3D::descriptor_translations(),
                    [
                        Op::selector(".poses[].position"),
                        Op::func(list_xyz_struct_to_list_fixed),
                    ],
                )
                .component(
                    InstancePoses3D::descriptor_quaternions(),
                    [
                        Op::selector(".poses[].orientation"),
                        Op::func(list_xyzw_struct_to_list_fixed),
                    ],
                )
            })?
            .build(),
    )
}
