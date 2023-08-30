//! Log some very simple points.
use rerun::{archetypes::Points3D, MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) =
        RecordingStreamBuilder::new("rerun_example_points3d_simple").memory()?;

    MsgSender::from_archetype("points", &Points3D::new([(0.0, 0.0, 0.0), (1.0, 1.0, 1.0)]))?
        .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
