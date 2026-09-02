use crate::{Lens, LensBuilderError};
use re_lenses_core::Selector;
use re_sdk_types::archetypes::{CoordinateFrame, Measurements};

use crate::semantic::helpers::constant_string;

/// Creates a lens for `sensor_msgs/msg/Temperature` messages.
pub fn temperature() -> Result<Lens, LensBuilderError> {
    Lens::derive("sensor_msgs.msg.Temperature:message")
        .to_component(
            CoordinateFrame::descriptor_frame(),
            Selector::parse(".header.frame_id")?,
        )
        .to_component(
            Measurements::descriptor_values(),
            Selector::parse(".temperature")?,
        )
        .to_component(
            Measurements::descriptor_variances(),
            Selector::parse(".variance")?,
        )
        .to_component(
            Measurements::descriptor_units(),
            Selector::parse(".")?.pipe(constant_string("°C")),
        )
        .build()
}
