use crate::{Lens, LensBuilderError};
use re_lenses_core::Selector;
use re_sdk_types::archetypes::{CoordinateFrame, Measurements};

/// Creates a lens for `sensor_msgs/msg/RelativeHumidity` messages.
///
/// The value is a dimensionless ratio in `[0.0, 1.0]`, so no unit is emitted.
pub fn relative_humidity() -> Result<Lens, LensBuilderError> {
    Lens::derive("sensor_msgs.msg.RelativeHumidity:message")
        .to_component(
            CoordinateFrame::descriptor_frame(),
            Selector::parse(".header.frame_id")?,
        )
        .to_component(
            Measurements::descriptor_values(),
            Selector::parse(".relative_humidity")?,
        )
        .to_component(
            Measurements::descriptor_variances(),
            Selector::parse(".variance")?,
        )
        .build()
}
