//! Log a couple 2D line segments using 2D line strips.

use rerun::{
    archetypes::{Boxes2D, LineStrips2D},
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_line_segments2d").memory()?;

    let points = [[0., 0.], [2., 1.], [4., -1.], [6., 0.]];
    rec.log("segments", &LineStrips2D::new(points.chunks(2)))?;

    // Log an extra rect to set the view bounds
    rec.log(
        "bounds",
        &Boxes2D::from_centers_and_sizes([(3.0, 0.0)], [(8.0, 6.0)]),
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
