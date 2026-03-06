use re_arrow_combinators::{Selector, Transform as _, map::MapList};
use re_lenses::{Lens, LensError, op};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::{CoordinateFrame, InstancePoses3D};

use crate::loader_mcap::lenses::helpers::{xyz_struct_to_fixed, xyzw_struct_to_fixed};

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
                    Selector::parse(".timestamp")?.then(MapList::new(op::timespec_to_nanos())),
                )?
                .component(
                    CoordinateFrame::descriptor_frame(),
                    Selector::parse(".frame_id")?,
                )?
                .component(
                    InstancePoses3D::descriptor_translations(),
                    Selector::parse(".poses[].position")?.then(MapList::new(xyz_struct_to_fixed())),
                )?
                .component(
                    InstancePoses3D::descriptor_quaternions(),
                    Selector::parse(".poses[].orientation")?
                        .then(MapList::new(xyzw_struct_to_fixed())),
                )
            })?
            .build(),
    )
}
