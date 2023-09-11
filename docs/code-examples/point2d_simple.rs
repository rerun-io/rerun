//! Log some very simple points.

use rerun::{
    archetypes::{Boxes2D, Points2D},
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_points2d").memory()?;

    rec.log("points", &Points2D::new([(0.0, 0.0), (1.0, 1.0)]))?;

    // Log an extra rect to set the view bounds
    rec.log("bounds", &Boxes2D::new([(2.0, 1.5)]))?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
