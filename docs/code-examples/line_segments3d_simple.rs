//! Log a simple set of line segments.
use rerun::{components::LineStrip3D, MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new(env!("CARGO_BIN_NAME")).memory()?;

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

    MsgSender::new("simple")
        .with_component(
            &points
                .chunks(2)
                .map(|p| LineStrip3D(vec![p[0].into(), p[1].into()]))
                .collect::<Vec<_>>(),
        )?
        .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
