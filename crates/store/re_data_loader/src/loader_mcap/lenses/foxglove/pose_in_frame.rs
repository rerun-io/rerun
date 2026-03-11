use re_lenses::{Lens, LensError, op};
use re_lenses_core::Selector;
use re_lenses_core::combinators::{MapList, Transform as _};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::{CoordinateFrame, InstancePoses3D};

use crate::loader_mcap::lenses::helpers::{xyz_struct_to_fixed, xyzw_struct_to_fixed};

use super::FOXGLOVE_TIMESTAMP;

/// Creates a lens for [`foxglove.PoseInFrame`] messages.
///
/// [`foxglove.PoseInFrame`]: https://docs.foxglove.dev/docs/sdk/schemas/pose-in-frame
pub fn pose_in_frame() -> Result<Lens, LensError> {
    Ok(
        Lens::for_input_column(EntityPathFilter::all(), "foxglove.PoseInFrame:message")
            .output_columns(|out| {
                out.time(
                    FOXGLOVE_TIMESTAMP,
                    TimeType::TimestampNs,
                    Selector::parse(".timestamp")?.then(MapList::new(op::timespec_to_nanos())),
                )?
                .component(
                    CoordinateFrame::descriptor_frame(),
                    Selector::parse(".frame_id")?,
                )?
                .component(
                    InstancePoses3D::descriptor_translations(),
                    Selector::parse(".pose.position")?.then(MapList::new(xyz_struct_to_fixed())),
                )?
                .component(
                    InstancePoses3D::descriptor_quaternions(),
                    Selector::parse(".pose.orientation")?
                        .then(MapList::new(xyzw_struct_to_fixed())),
                )
            })?
            .build(),
    )
}
