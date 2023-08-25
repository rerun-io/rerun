//! Log some a single oriented bounding box
use rerun::components::Box3D;
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new("rerun-example-box3d").memory()?;

    MsgSender::new("simple")
        .with_component(&[Box3D::new(2.0, 2.0, 1.0)])?
        .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
