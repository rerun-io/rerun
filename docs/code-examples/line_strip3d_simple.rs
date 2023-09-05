//! Log a simple line strip.

use rerun::{archetypes::LineStrips3D, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_line_strip3d").memory()?;

    let points = [
        [0., 0., 0.],
        [0., 0., 1.],
        [1., 0., 0.],
        [1., 0., 1.],
        [1., 1., 0.],
        [1., 1., 1.],
        [0., 1., 0.],
        [0., 1., 1.],
    ];
    rec.log("strip", &LineStrips3D::new([points]))?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
