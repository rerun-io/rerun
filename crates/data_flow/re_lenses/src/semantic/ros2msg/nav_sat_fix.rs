use crate::{Lens, LensBuilderError};
use re_lenses_core::Selector;
use re_sdk_types::ComponentDescriptor;
use re_sdk_types::archetypes::{CoordinateFrame, GeoPoints};

use crate::semantic::helpers::lat_lon_struct_to_fixed;

/// Creates a lens for `sensor_msgs/msg/NavSatFix` messages.
pub fn nav_sat_fix() -> Result<Lens, LensBuilderError> {
    Lens::derive("sensor_msgs.msg.NavSatFix:message")
        .to_component(
            CoordinateFrame::descriptor_frame(),
            Selector::parse(".header.frame_id")?,
        )
        .to_component(
            GeoPoints::descriptor_positions(),
            Selector::parse(".")?.pipe(lat_lon_struct_to_fixed()),
        )
        .to_component(
            // TODO(michael): use a common archetype for altitude?
            ComponentDescriptor::partial("altitude")
                .with_archetype("sensor_msgs.msg.NavSatFix".into()),
            Selector::parse(".altitude")?,
        )
        .build()
    // TODO(michael): status & covariance aren't supported by Rerun's `GeoPoints`. Add them?
}
