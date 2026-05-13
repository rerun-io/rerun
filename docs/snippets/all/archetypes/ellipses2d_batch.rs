//! Log a batch of 2D ellipses.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_ellipses2d_batch").spawn()?;

    rec.log(
        "batch",
        &rerun::Ellipses2D::from_centers_and_half_sizes(
            [(-2.0, 0.0), (0.0, 0.0), (2.5, 0.0)],
            [(1.5, 0.75), (0.5, 0.5), (0.75, 1.5)],
        )
        .with_line_radii([0.025, 0.05, 0.025])
        .with_colors([
            rerun::Color::from_rgb(255, 0, 0),
            rerun::Color::from_rgb(0, 255, 0),
            rerun::Color::from_rgb(0, 0, 255),
        ])
        .with_labels(["wide", "circle", "tall"]),
    )?;

    Ok(())
}
