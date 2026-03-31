use re_lenses::{Lens, LensError, op};
use re_lenses_core::Selector;
use re_lenses_core::combinators::{Flatten, MapList, Transform as _};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::{CoordinateFrame, InstancePoses3D, Points3D};

use crate::loader_mcap::lenses::helpers::{xyz_struct_to_fixed, xyzw_struct_to_fixed};

use super::FOXGLOVE_TIMESTAMP;
use super::packed_element_field::{ExtractColors, ExtractPositions};

/// Creates a lens for [`foxglove.PointCloud`] messages.
///
/// [`foxglove.PointCloud`]: https://docs.foxglove.dev/docs/sdk/schemas/point-cloud
pub fn point_cloud(time_type: TimeType) -> Result<Lens, LensError> {
    Ok(
        Lens::for_input_column(EntityPathFilter::all(), "foxglove.PointCloud:message")
            .output_columns(|out| {
                out.time(
                    FOXGLOVE_TIMESTAMP,
                    time_type,
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
                )?
                // The pose field is optional.
                .component(
                    InstancePoses3D::descriptor_translations(),
                    Selector::parse(".pose.position!")?.then(MapList::new(xyz_struct_to_fixed())),
                )?
                .component(
                    InstancePoses3D::descriptor_quaternions(),
                    Selector::parse(".pose.orientation!")?
                        .then(MapList::new(xyzw_struct_to_fixed())),
                )
            })?
            .build(),
    )
}
