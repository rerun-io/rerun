//! Use the `send_columns` API to send several point clouds over time in a single call.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_send_columns_arrays").spawn()?;

    #[rustfmt::skip]
    let positions = [
        [[1.0, 0.0, 1.0], [0.5, 0.5, 2.0]],
        [[1.5, -0.5, 1.5], [1.0, 1.0, 2.5], [-0.5, 1.5, 1.0], [-1.5, 0.0, 2.0]],
        [[2.0, 0.0, 2.0], [1.5, -1.5, 3.0], [0.0, -2.0, 2.5], [1.0, -1.0, 3.5]],
        [[-2.0, 0.0, 2.0], [-1.5, 1.5, 3.0], [-1.0, 1.0, 3.5]],
        [[1.0, -1.0, 1.0], [2.0, -2.0, 2.0], [3.0, -1.0, 3.0], [2.0, 0.0, 4.0]],
    ];

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

        rec.set_time_sequence("frame", i);
        rec.log(
            "points",
            &rerun::Points3D::update_fields()
                .with_radii(radii)
                .with_colors(colors),
        )?;
    }

    rec.set_time_sequence("frame", 20);
    rec.log(
        "points",
        &rerun::Points3D::clear_fields()
            .with_positions(positions())
            .with_radii([0.3]),
    )?;

    Ok(())
}
