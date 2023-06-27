//! Log a simple set of line segments.
use rerun::components::LineStrip3D;
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new("linestrip2d").memory()?;

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
        .with_component(
            &points
                .chunks(2)
                .map(|p| LineStrip3D(vec![p[0].into(), p[1].into()]))
                .collect::<Vec<_>>(),
        )?
        .send(&rec_stream)?;

    rec_stream.flush_blocking();
    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
