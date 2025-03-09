//! Update a point cloud over time, in a single operation.
//!
//! This is semantically equivalent to the `points3d_row_updates` example, albeit much faster.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_points3d_column_updates").spawn()?;

    let times = rerun::TimeColumn::new_duration_seconds("time", 10..15);

    // Prepare a point cloud that evolves over 5 timesteps, changing the number of points in the process.
    #[rustfmt::skip]
    let positions = [
        [1.0, 0.0, 1.0], [0.5, 0.5, 2.0],
        [1.5, -0.5, 1.5], [1.0, 1.0, 2.5], [-0.5, 1.5, 1.0], [-1.5, 0.0, 2.0],
        [2.0, 0.0, 2.0], [1.5, -1.5, 3.0], [0.0, -2.0, 2.5], [1.0, -1.0, 3.5],
        [-2.0, 0.0, 2.0], [-1.5, 1.5, 3.0], [-1.0, 1.0, 3.5],
        [1.0, -1.0, 1.0], [2.0, -2.0, 2.0], [3.0, -1.0, 3.0], [2.0, 0.0, 4.0],
    ];

    // At each timestep, all points in the cloud share the same but changing color and radius.
    let colors = [0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF];
    let radii = [0.05, 0.01, 0.2, 0.1, 0.3];

    // Partition our data as expected across the 5 timesteps.
    let position = rerun::Points3D::update_fields()
        .with_positions(positions)
        .columns([2, 4, 4, 3, 4])?;
    let color_and_radius = rerun::Points3D::update_fields()
        .with_colors(colors)
        .with_radii(radii)
        .columns_of_unit_batches()?;

    rec.send_columns("points", [times], position.chain(color_and_radius))?;

    Ok(())
}
