//! Log a simple line strip.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_line_strip2d").spawn()?;

    let points = [[0., 0.], [2., 1.], [4., -1.], [6., 0.]];
    rec.log("strip", &rerun::LineStrips2D::new([points]))?;

    // Log an extra rect to set the view bounds
    rec.log(
        "bounds",
        &rerun::Boxes2D::from_centers_and_sizes([(3., 0.)], [(8., 6.)]),
    )?;

    Ok(())
}
