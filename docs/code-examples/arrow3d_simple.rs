//! Log a single arrow.

use rerun::{
    components::{Radius, Vector3D},
    MsgSender, RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new("rerun_example_arrow3d").memory()?;

    let arrow = Vector3D::from((0.0, 1.0, 0.0));

    MsgSender::new("arrow")
        .with_component(&[arrow])?
        .with_component(&[Radius(0.05)])?
        .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
