use re_lenses::{CastTo, Lens, LensBuilderError};
use re_lenses_core::Selector;
use re_sdk_types::archetypes::{CoordinateFrame, InstancePoses3D};

/// Creates a lens for `geometry_msgs/msg/PoseStamped` messages.
pub fn pose_stamped() -> Result<Lens, LensBuilderError> {
    Lens::derive("geometry_msgs.msg.PoseStamped:message")
        .to_component(
            CoordinateFrame::descriptor_frame(),
            Selector::parse(".header.frame_id")?,
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
