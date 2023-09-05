//! Log some very simple points.

use rerun::{archetypes::Points3D, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_points3d_simple").memory()?;

    rec.log("points", &Points3D::new([(0.0, 0.0, 0.0), (1.0, 1.0, 1.0)]))?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
