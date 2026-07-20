use re_lenses::{CastTo, Lens, LensBuilderError, op};
use re_lenses_core::Selector;
use re_log_types::TimeType;
use re_sdk_types::archetypes::{CoordinateFrame, InstancePoses3D};

use super::FOXGLOVE_TIMESTAMP;

/// Creates a lens for [`foxglove.PoseInFrame`] messages.
///
/// [`foxglove.PoseInFrame`]: https://docs.foxglove.dev/docs/sdk/schemas/pose-in-frame
pub fn pose_in_frame(time_type: TimeType) -> Result<Lens, LensBuilderError> {
    Lens::derive("foxglove.PoseInFrame:message")
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
            Selector::parse(".pose.position | pack(.x!, .y!, .z!)")?,
            CastTo::Auto,
        )
        .to_component_with_cast(
            InstancePoses3D::descriptor_quaternions(),
            Selector::parse(".pose.orientation | pack(.x!, .y!, .z!, .w!)")?,
            CastTo::Auto,
        )
        .build()
}
