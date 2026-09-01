use crate::{CastTo, Lens, LensBuilderError};
use re_lenses_core::Selector;
use re_sdk_types::archetypes::Transform3D;

/// Creates a lens for `tf2_msgs/msg/TFMessage` messages.
pub fn tf_message() -> Result<Lens, LensBuilderError> {
    Lens::scatter("tf2_msgs.msg.TFMessage:message")
        .to_component(
            Transform3D::descriptor_parent_frame(),
            Selector::parse(".transforms[].header.frame_id")?,
        )
        .to_component(
            Transform3D::descriptor_child_frame(),
            Selector::parse(".transforms[].child_frame_id")?,
        )
        .to_component_with_cast(
            Transform3D::descriptor_translation(),
            Selector::parse(".transforms[].transform.translation | pack(.x!, .y!, .z!)")?,
            CastTo::Auto,
        )
        .to_component_with_cast(
            Transform3D::descriptor_quaternion(),
            Selector::parse(".transforms[].transform.rotation | pack(.x!, .y!, .z!, .w!)")?,
            CastTo::Auto,
        )
        .build()
}
