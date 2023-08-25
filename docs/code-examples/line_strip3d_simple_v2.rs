//! Log a simple line strip.

use rerun::{archetypes::LineStrips3D, MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) =
        RecordingStreamBuilder::new("rerun-example-line_strip3d").memory()?;

    let points = [
        [0., 0., 0.],
        [0., 0., 1.],
        [1., 0., 0.],
        [1., 0., 1.],
        [1., 1., 0.],
        [1., 1., 1.],
        [0., 1., 0.],
        [0., 1., 1.],
    ];

    MsgSender::from_archetype("strip", &LineStrips3D::new([points]))?.send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
