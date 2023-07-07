//! Log a single arrow
use rerun::components::{Arrow3D, Radius, Vec3D};
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new("arrow").memory()?;

    let arrow = Arrow3D {
        origin: Vec3D::from([0.0, 0.0, 0.0]),
        vector: Vec3D::from([1.0, 0.0, 1.0]),
    };

    MsgSender::new("arrow")
        .with_component(&[arrow])?
        .with_component(&[Radius(0.05)])?
        .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
