//! Log a simple set of line segments.

use rerun::{archetypes::LineStrips3D, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_line_segments3d").memory()?;

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
    rec.log("segments", &LineStrips3D::new(points.chunks(2)))?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
