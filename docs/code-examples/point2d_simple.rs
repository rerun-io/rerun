//! Log some very simple points.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = rerun::RecordingStreamBuilder::new("rerun_example_points2d").memory()?;

    rec.log("points", &rerun::Points2D::new([(0.0, 0.0), (1.0, 1.0)]))?;

    // Log an extra rect to set the view bounds
    rec.log("bounds", &rerun::Boxes2D::from_half_sizes([(2.0, 1.5)]))?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
