use re_lenses::{CastTo, Lens, LensBuilderError, op};
use re_lenses_core::Selector;
use re_log_types::TimeType;
use re_sdk_types::archetypes::Transform3D;

use super::FOXGLOVE_TIMESTAMP;

/// Creates a lens for [`foxglove.FrameTransform`] messages.
///
/// [`foxglove.FrameTransform`]: https://docs.foxglove.dev/docs/sdk/schemas/frame-transform
pub fn frame_transform(time_type: TimeType) -> Result<Lens, LensBuilderError> {
    Lens::scatter("foxglove.FrameTransform:message")
        .to_timeline(
            FOXGLOVE_TIMESTAMP,
            time_type,
            Selector::parse(".timestamp")?.pipe(op::timespec_to_nanos()),
        )
        .to_component(
            Transform3D::descriptor_parent_frame(),
            Selector::parse(".parent_frame_id")?,
        )
        .to_component(
            Transform3D::descriptor_child_frame(),
            Selector::parse(".child_frame_id")?,
        )
        .to_component_with_cast(
            Transform3D::descriptor_translation(),
            Selector::parse(".translation | pack(.x!, .y!, .z!)")?,
            CastTo::Auto,
        )
        .to_component_with_cast(
            Transform3D::descriptor_quaternion(),
            Selector::parse(".rotation | pack(.x!, .y!, .z!, .w!)")?,
            CastTo::Auto,
        )
        .build()
}
