use re_lenses::{Lens, LensError, op};
use re_lenses_core::Selector;
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::{CoordinateFrame, GeoPoints};

use crate::importer_mcap::lenses::helpers::lat_lon_struct_to_fixed;

use super::FOXGLOVE_TIMESTAMP;

/// Creates a lens for [`foxglove.LocationFixes`] messages.
///
/// Each fix in the batch gets its own timestamp, position, color, and coordinate frame.
///
/// [`foxglove.LocationFixes`]: https://docs.foxglove.dev/docs/sdk/schemas/location-fixes
pub fn location_fixes(time_type: TimeType) -> Result<Lens, LensError> {
    Ok(
        Lens::for_input_column(EntityPathFilter::all(), "foxglove.LocationFixes:message")
            .output_scatter_columns(|out| {
                out.time(
                    FOXGLOVE_TIMESTAMP,
                    time_type,
                    Selector::parse(".fixes[].timestamp")?.pipe(op::timespec_to_nanos()),
                )?
                // `frame_id` can be missing in old versions of the schema.
                .component(
                    CoordinateFrame::descriptor_frame(),
                    Selector::parse(".fixes[].frame_id!")?,
                )?
                .component(
                    GeoPoints::descriptor_positions(),
                    Selector::parse(".fixes[]")?.pipe(lat_lon_struct_to_fixed()),
                )?
                // `color` field is optional.
                .component(
                    GeoPoints::descriptor_colors(),
                    Selector::parse(".fixes[].color!")?.pipe(op::rgba_struct_to_uint32()),
                )
            })?
            .build(),
    )
}
