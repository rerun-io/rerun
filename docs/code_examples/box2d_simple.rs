//! Log some very simple 2D boxes.

use rerun::{archetypes::Boxes2D, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_box2d").memory()?;

    rec.log(
        "simple",
        &Boxes2D::from_mins_and_sizes([(-1., -1.)], [(2., 2.)]),
    )?;

    // Log an extra rect to set the view bounds
    rec.log("bounds", &Boxes2D::from_sizes([(4., 3.)]))?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
