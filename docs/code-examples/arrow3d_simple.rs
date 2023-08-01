//! Log a single arrow
use rerun::{components::Radius, datatypes::Vec3D, MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new("arrow").memory()?;

    let arrow = rerun::components::Arrow3D::new(Vec3D::ZERO, (0.0, 1.0, 0.0));

    MsgSender::new("arrow")
        .with_component(&[arrow])?
        .with_component(&[Radius(0.05)])?
        .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
