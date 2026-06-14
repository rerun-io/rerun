//! Log a simple sparse voxel grid map.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new(
        "rerun_example_voxel_grid_map_simple",
    )
    .spawn()?;

    let voxel_indices = [
        (-1, 0, 0),
        (1, 0, 0),
        (1, 1, 0),
        (3, 0, 0),
        (3, 0, 1),
        (4, 0, 1),
    ];
    let values = [0.0_f32, 0.2, 0.4, 0.6, 0.8, 1.0];

    rec.log(
        "world/voxels",
        &rerun::VoxelGridMap::new(voxel_indices, [0.25, 0.25, 0.25])
            .with_values(values)
            .with_value_range([0.0, 1.0])
            .with_colormap(rerun::components::Colormap::Turbo)
            .with_translation([-0.5, -0.5, 0.0]),
    )?;

    Ok(())
}
