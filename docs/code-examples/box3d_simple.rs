//! Log some a single oriented bounding box
use rerun::components::Box3D;
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_box3d").memory()?;

    MsgSender::new("simple")
        .with_component(&[Box3D::new(2.0, 2.0, 1.0)])?
        .send(&rec)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
