//! Log a simple line strip.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_line_strip2d").spawn()?;

    let points = [[0., 0.], [2., 1.], [4., -1.], [6., 0.]];
    rec.log("strip", &rerun::LineStrips2D::new([points]))?;

    // TODO(#5521): log VisualBounds

    Ok(())
}
