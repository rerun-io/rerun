//! Log a simple line strip.

use rerun::components::LineStrip3D;
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) =
        RecordingStreamBuilder::new("rerun-example-line_strip3d").memory()?;

    let points = vec![
        [0., 0., 0.],
        [0., 0., 1.],
        [1., 0., 0.],
        [1., 0., 1.],
        [1., 1., 0.],
        [1., 1., 1.],
        [0., 1., 0.],
        [0., 1., 1.],
    ];

    MsgSender::new("simple")
        .with_component(&[LineStrip3D(points.into_iter().map(Into::into).collect())])?
        .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
