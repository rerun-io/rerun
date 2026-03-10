use re_arrow_combinators::{Selector, Transform as _, map::MapList};
use re_lenses::{Lens, LensError, op};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::Transform3D;

use crate::loader_mcap::lenses::helpers::{xyz_struct_to_fixed, xyzw_struct_to_fixed};

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
                    Selector::parse(".transforms[].timestamp")?
                        .then(MapList::new(op::timespec_to_nanos())),
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
                        .then(MapList::new(xyz_struct_to_fixed())),
                )?
                .component(
                    Transform3D::descriptor_quaternion(),
                    Selector::parse(".transforms[].rotation")?
                        .then(MapList::new(xyzw_struct_to_fixed())),
                )
            })?
            .build(),
    )
}
