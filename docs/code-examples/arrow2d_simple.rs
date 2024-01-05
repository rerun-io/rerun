//! Log a batch of 2D arrows.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_arrow2d").spawn()?;

    rec.log(
        "arrows",
        &rerun::Arrows2D::from_vectors([[1.0, 0.0], [0.0, -1.0], [-0.7, 0.7]])
            .with_radii([0.025])
            .with_origins([rerun::Position2D::ZERO])
            .with_colors([[255, 0, 0], [0, 255, 0], [127, 0, 255]])
            .with_labels(["right", "up", "left-down"]),
    )?;

    Ok(())
}
