//! Log some very simple points.

use rerun::{components::Point3D, MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_points3d").memory()?;

    let points = [[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]]
        .into_iter()
        .map(Point3D::from)
        .collect::<Vec<_>>();

    MsgSender::new("points")
        .with_component(&points)?
        .send(&rec)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
