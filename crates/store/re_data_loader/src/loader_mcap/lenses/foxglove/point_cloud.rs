use re_lenses::{Lens, LensError, Op};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::{CoordinateFrame, Points3D};

use super::FOXGLOVE_TIMESTAMP;
use super::packed_element_field::{extract_colors, extract_positions};

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
                    [Op::selector(".timestamp"), Op::time_spec_to_nanos()],
                )
                .component(
                    CoordinateFrame::descriptor_frame(),
                    [Op::selector(".frame_id")],
                )
                .component(
                    Points3D::descriptor_positions(),
                    [Op::func(extract_positions)],
                )
                .component(Points3D::descriptor_colors(), [Op::func(extract_colors)])
            })?
            .build(),
    )
}
