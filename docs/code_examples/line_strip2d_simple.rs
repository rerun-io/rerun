//! Log a simple line strip.

use rerun::{
    archetypes::{Boxes2D, LineStrips2D},
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_line_strip2d").memory()?;

    let points = [[0., 0.], [2., 1.], [4., -1.], [6., 0.]];
    rec.log("strip", &LineStrips2D::new([points]))?;

    // Log an extra rect to set the view bounds
    rec.log(
        "bounds",
        &Boxes2D::from_centers_and_sizes([(3., 0.)], [(8., 6.)]),
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
