use re_lenses::{Lens, LensError, op};
use re_lenses_core::Selector;
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::{CoordinateFrame, InstancePoses3D, Points3D};

use super::FOXGLOVE_TIMESTAMP;
use super::packed_element_field::{extract_colors, extract_positions};

/// Creates a lens for [`foxglove.PointCloud`] messages.
///
/// [`foxglove.PointCloud`]: https://docs.foxglove.dev/docs/sdk/schemas/point-cloud
pub fn point_cloud(time_type: TimeType) -> Result<Lens, LensError> {
    let flatten = Selector::parse(".[]")?;

    Ok(
        Lens::for_input_column(EntityPathFilter::all(), "foxglove.PointCloud:message")
            .output_columns(|out| {
                out.time(
                    FOXGLOVE_TIMESTAMP,
                    time_type,
                    Selector::parse(".timestamp")?.pipe(op::timespec_to_nanos()),
                )?
                .component(
                    CoordinateFrame::descriptor_frame(),
                    Selector::parse(".frame_id")?,
                )?
                .component(
                    Points3D::descriptor_positions(),
                    // Each message contains a variable number of packed points, so
                    // `extract_positions` returns a `List<FixedSizeList<f32, 3>>`.
                    // The `.[]` flatten unwraps this extra list level so the component
                    // column contains the points directly.
                    Selector::parse(".")?
                        .pipe(extract_positions)
                        .pipe(flatten.clone()),
                )?
                .component(
                    Points3D::descriptor_colors(),
                    // Each message contains a variable number of packed colors, so
                    // `extract_colors` returns a `List<UInt32>`. The `.[]` flatten
                    // unwraps this extra list level so the component column contains
                    // the colors directly.
                    Selector::parse(".")?.pipe(extract_colors).pipe(flatten),
                )?
                // The pose field is optional.
                .component(
                    InstancePoses3D::descriptor_translations(),
                    Selector::parse(".pose.position!")?
                        .pipe(op::struct_to_fixed_size_list_f32(["x", "y", "z"])),
                )?
                .component(
                    InstancePoses3D::descriptor_quaternions(),
                    Selector::parse(".pose.orientation!")?
                        .pipe(op::struct_to_fixed_size_list_f32(["x", "y", "z", "w"])),
                )
            })?
            .build(),
    )
}
