//! Update a point cloud over time.
//!
//! See also the `points3d_column_updates` example, which achieves the same thing in a single operation.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_points3d_row_updates").spawn()?;

    // Prepare a point cloud that evolves over 5 timesteps, changing the number of points in the process.
    #[rustfmt::skip]
    let positions = [
        vec![[1.0, 0.0, 1.0], [0.5, 0.5, 2.0]],
        vec![[1.5, -0.5, 1.5], [1.0, 1.0, 2.5], [-0.5, 1.5, 1.0], [-1.5, 0.0, 2.0]],
        vec![[2.0, 0.0, 2.0], [1.5, -1.5, 3.0], [0.0, -2.0, 2.5], [1.0, -1.0, 3.5]],
        vec![[-2.0, 0.0, 2.0], [-1.5, 1.5, 3.0], [-1.0, 1.0, 3.5]],
        vec![[1.0, -1.0, 1.0], [2.0, -2.0, 2.0], [3.0, -1.0, 3.0], [2.0, 0.0, 4.0]],
    ];

    // At each timestep, all points in the cloud share the same but changing color and radius.
    let colors = [0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF];
    let radii = [0.05, 0.01, 0.2, 0.1, 0.3];

    for (time, positions, color, radius) in itertools::izip!(10..15, positions, colors, radii) {
        rec.set_duration_secs("time", time);

        let point_cloud = rerun::Points3D::new(positions)
            .with_colors([color])
            .with_radii([radius]);

        rec.log("points", &point_cloud)?;
    }

    Ok(())
}
