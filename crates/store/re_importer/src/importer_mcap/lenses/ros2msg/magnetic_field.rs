use re_lenses::{CastTo, Lens, LensBuilderError};
use re_lenses_core::Selector;
use re_sdk_types::archetypes::{Arrows3D, CoordinateFrame};

/// Creates a lens for `sensor_msgs/msg/MagneticField` messages.
// TODO(isaac): support also `magnetic_field_covariance`.
pub fn magnetic_field() -> Result<Lens, LensBuilderError> {
    Lens::derive("sensor_msgs.msg.MagneticField:message")
        .to_component(
            CoordinateFrame::descriptor_frame(),
            Selector::parse(".header.frame_id")?,
        )
        .to_component_with_cast(
            Arrows3D::descriptor_vectors(),
            Selector::parse(".magnetic_field | pack(.x!, .y!, .z!)")?,
            CastTo::Auto,
        )
        .build()
}
