use re_lenses::{Lens, LensBuilderError, op};
use re_lenses_core::Selector;
use re_log_types::TimeType;
use re_sdk_types::archetypes::{CoordinateFrame, VoxelGridMap};

use super::FOXGLOVE_TIMESTAMP;
use super::packed_element_field::{extract_colors, extract_voxel_indices};

/// Creates a lens for [`foxglove.VoxelGrid`] messages.
///
/// [`foxglove.VoxelGrid`]: https://docs.foxglove.dev/docs/sdk/schemas/voxel-grid
pub fn voxel_grid(time_type: TimeType) -> Result<Lens, LensBuilderError> {
    let flatten = Selector::parse(".[]")?;

    Lens::derive("foxglove.VoxelGrid:message")
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
            VoxelGridMap::descriptor_voxel_indices(),
            // Foxglove voxel grids are densely packed in Z-Y-X order. Convert
            // each message to sparse Rerun voxel indices, then unwrap the
            // per-message list level into the component column.
            Selector::parse(".")?
                .pipe(extract_voxel_indices)
                .pipe(flatten.clone()),
        )
        .to_component(
            VoxelGridMap::descriptor_voxel_size(),
            Selector::parse(".cell_size")?.pipe(op::struct_to_fixed_size_list_f32(["x", "y", "z"])),
        )
        .to_component(
            VoxelGridMap::descriptor_colors(),
            Selector::parse(".")?
                .pipe(extract_colors("cell_stride"))
                .pipe(flatten),
        )
        .to_component(
            VoxelGridMap::descriptor_translation(),
            Selector::parse(".pose.position!")?
                .pipe(op::struct_to_fixed_size_list_f32(["x", "y", "z"])),
        )
        .to_component(
            VoxelGridMap::descriptor_quaternion(),
            Selector::parse(".pose.orientation!")?
                .pipe(op::struct_to_fixed_size_list_f32(["x", "y", "z", "w"])),
        )
        .build()
}
