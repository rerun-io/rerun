use crate::{Lens, LensBuilderError};
use re_lenses_core::Selector;
use re_sdk_types::archetypes::{CoordinateFrame, Measurements};

use crate::semantic::helpers::constant_string;

/// Creates a lens for `sensor_msgs/msg/Illuminance` messages.
pub fn illuminance() -> Result<Lens, LensBuilderError> {
    Lens::derive("sensor_msgs.msg.Illuminance:message")
        .to_component(
            CoordinateFrame::descriptor_frame(),
            Selector::parse(".header.frame_id")?,
        )
        .to_component(
            Measurements::descriptor_values(),
            Selector::parse(".illuminance")?,
        )
        .to_component(
            Measurements::descriptor_variances(),
            Selector::parse(".variance")?,
        )
        .to_component(
            Measurements::descriptor_units(),
            Selector::parse(".")?.pipe(constant_string("lux")),
        )
        .build()
}
