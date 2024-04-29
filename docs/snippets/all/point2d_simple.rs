//! Log some very simple points.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_points2d").spawn()?;

    rec.log("points", &rerun::Points2D::new([(0.0, 0.0), (1.0, 1.0)]))?;

    // TODO(#5521): log VisualBounds

    Ok(())
}
