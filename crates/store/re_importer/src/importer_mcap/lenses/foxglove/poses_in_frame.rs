use re_lenses::{CastTo, Lens, LensBuilderError, op};
use re_lenses_core::Selector;
use re_log_types::TimeType;
use re_sdk_types::archetypes::{CoordinateFrame, InstancePoses3D};

use super::FOXGLOVE_TIMESTAMP;

/// Creates a lens for [`foxglove.PosesInFrame`] messages.
///
/// [`foxglove.PosesInFrame`]: https://docs.foxglove.dev/docs/sdk/schemas/poses-in-frame
pub fn poses_in_frame(time_type: TimeType) -> Result<Lens, LensBuilderError> {
    Lens::derive("foxglove.PosesInFrame:message")
        .to_timeline(
            FOXGLOVE_TIMESTAMP,
            time_type,
            Selector::parse(".timestamp")?.pipe(op::timespec_to_nanos()),
        )
        .to_component(
            CoordinateFrame::descriptor_frame(),
            Selector::parse(".frame_id")?,
        )
        .to_component_with_cast(
            InstancePoses3D::descriptor_translations(),
            Selector::parse(".poses[].position | pack(.x!, .y!, .z!)")?,
            CastTo::Auto,
        )
        .to_component_with_cast(
            InstancePoses3D::descriptor_quaternions(),
            Selector::parse(".poses[].orientation | pack(.x!, .y!, .z!, .w!)")?,
            CastTo::Auto,
        )
        .build()
}
