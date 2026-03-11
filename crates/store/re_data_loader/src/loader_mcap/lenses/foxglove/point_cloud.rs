use re_lenses::{Lens, LensError, op};
use re_lenses_core::Selector;
use re_lenses_core::combinators::{Flatten, MapList, Transform as _};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::{CoordinateFrame, Points3D};

use super::FOXGLOVE_TIMESTAMP;
use super::packed_element_field::{ExtractColors, ExtractPositions};

/// Creates a lens for [`foxglove.PointCloud`] messages.
///
/// [`foxglove.PointCloud`]: https://docs.foxglove.dev/docs/sdk/schemas/point-cloud
pub fn point_cloud() -> Result<Lens, LensError> {
    Ok(
        // TODO(michael): support optional pose field (RR-3766).
        Lens::for_input_column(EntityPathFilter::all(), "foxglove.PointCloud:message")
            .output_columns(|out| {
                out.time(
                    FOXGLOVE_TIMESTAMP,
                    TimeType::TimestampNs,
                    Selector::parse(".timestamp")?.then(MapList::new(op::timespec_to_nanos())),
                )?
                .component(
                    CoordinateFrame::descriptor_frame(),
                    Selector::parse(".frame_id")?,
                )?
                .component(
                    Points3D::descriptor_positions(),
                    Selector::parse(".")?
                        .then(MapList::new(ExtractPositions))
                        .then(Flatten::new()),
                )?
                .component(
                    Points3D::descriptor_colors(),
                    Selector::parse(".")?
                        .then(MapList::new(ExtractColors))
                        .then(Flatten::new()),
                )
            })?
            .build(),
    )
}
