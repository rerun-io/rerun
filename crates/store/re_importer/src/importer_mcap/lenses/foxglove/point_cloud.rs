use re_lenses::{CastTo, Lens, LensBuilderError, op};
use re_lenses_core::Selector;
use re_log_types::TimeType;
use re_sdk_types::archetypes::{CoordinateFrame, InstancePoses3D, Points3D};

use super::FOXGLOVE_TIMESTAMP;
use super::packed_element_field::{extract_colors, extract_positions};

/// Creates a lens for [`foxglove.PointCloud`] messages.
///
/// [`foxglove.PointCloud`]: https://docs.foxglove.dev/docs/sdk/schemas/point-cloud
pub fn point_cloud(time_type: TimeType) -> Result<Lens, LensBuilderError> {
    let flatten = Selector::parse(".[]")?;

    Lens::derive("foxglove.PointCloud:message")
        .to_timeline(
            FOXGLOVE_TIMESTAMP,
            time_type,
            Selector::parse(".timestamp")?.pipe(op::timespec_to_nanos()),
        )
        .to_component(
            CoordinateFrame::descriptor_frame(),
            Selector::parse(".frame_id")?,
        )
        .to_component(
            Points3D::descriptor_positions(),
            // Each message contains a variable number of packed points, so
            // `extract_positions` returns a `List<FixedSizeList<f32, 3>>`.
            // The `.[]` flatten unwraps this extra list level so the component
            // column contains the points directly.
            Selector::parse(".")?
                .pipe(extract_positions)
                .pipe(flatten.clone()),
        )
        .to_component(
            Points3D::descriptor_colors(),
            // Each message contains a variable number of packed colors, so
            // `extract_colors` returns a `List<UInt32>`. The `.[]` flatten
            // unwraps this extra list level so the component column contains
            // the colors directly.
            Selector::parse(".")?
                .pipe(extract_colors("point_stride"))
                .pipe(flatten),
        )
        // The pose field is optional.
        .to_component_with_cast(
            InstancePoses3D::descriptor_translations(),
            Selector::parse(".pose.position! | pack(.x!, .y!, .z!)")?,
            CastTo::Auto,
        )
        .to_component_with_cast(
            InstancePoses3D::descriptor_quaternions(),
            Selector::parse(".pose.orientation! | pack(.x!, .y!, .z!, .w!)")?,
            CastTo::Auto,
        )
        .build()
}
