//! Update specific properties of a point cloud over time.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_points3d_partial_updates").spawn()?;

    let positions = || (0..10).map(|i| (i as f32, 0.0, 0.0));

    rec.set_time_sequence("frame", 0);
    rec.log("points", &rerun::Points3D::new(positions()))?;

    for i in 0..10 {
        let colors = (0..10).map(|n| {
            if n < i {
                rerun::Color::from_rgb(20, 200, 20)
            } else {
                rerun::Color::from_rgb(200, 20, 20)
            }
        });
        let radii = (0..10).map(|n| if n < i { 0.6 } else { 0.2 });

        // Update only the colors and radii, leaving everything else as-is.
        rec.set_time_sequence("frame", i);
        rec.log(
            "points",
            &rerun::Points3D::update_fields()
                .with_radii(radii)
                .with_colors(colors),
        )?;
    }

    // Update the positions and radii, and clear everything else in the process.
    rec.set_time_sequence("frame", 20);
    rec.log(
        "points",
        &rerun::Points3D::clear_fields()
            .with_positions(positions())
            .with_radii([0.3]),
    )?;

    Ok(())
}
