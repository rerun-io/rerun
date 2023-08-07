//! Log some very simple points.

use rerun::{components::Point3D, MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new(env!("CARGO_BIN_NAME")).memory()?;

    let points = [[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]]
        .into_iter()
        .map(Point3D::from)
        .collect::<Vec<_>>();

    MsgSender::new("points")
        .with_component(&points)?
        .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
