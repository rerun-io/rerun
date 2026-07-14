use re_lenses::{Lens, LensBuilderError, op};
use re_lenses_core::Selector;
use re_log_types::TimeType;
use re_sdk_types::archetypes::{CoordinateFrame, GeoPoints};

use crate::importer_mcap::lenses::helpers::lat_lon_struct_to_fixed;

use super::FOXGLOVE_TIMESTAMP;

/// Creates a lens for [`foxglove.LocationFix`] messages.
///
/// Converts a single GNSS fix to a [`GeoPoints`] archetype with one position and optional color.
///
/// [`foxglove.LocationFix`]: https://docs.foxglove.dev/docs/sdk/schemas/location-fix
pub fn location_fix(time_type: TimeType) -> Result<Lens, LensBuilderError> {
    Lens::derive("foxglove.LocationFix:message")
        .to_timeline(
            FOXGLOVE_TIMESTAMP,
            time_type,
            Selector::parse(".timestamp")?.pipe(op::timespec_to_nanos()),
        )
        // `frame_id` can be missing in old versions of the schema.
        .to_component(
            CoordinateFrame::descriptor_frame(),
            Selector::parse(".frame_id!")?,
        )
        .to_component(
            GeoPoints::descriptor_positions(),
            Selector::parse(".")?.pipe(lat_lon_struct_to_fixed()),
        )
        // `color` field is optional.
        .to_component(
            GeoPoints::descriptor_colors(),
            Selector::parse(".color!")?.pipe(op::rgba_struct_to_uint32()),
        )
        .build()
}
