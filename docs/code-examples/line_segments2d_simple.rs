//! Log a simple line segment.
use rerun::components::{LineStrip2D, Rect2D, Vec4D};
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new("linestrip2d").memory()?;

    let points = vec![[0., 0.], [2., 1.], [4., -1.], [6., 0.]];

    MsgSender::new("simple")
        .with_component(
            &points
                .chunks(2)
                .map(|p| LineStrip2D(vec![p[0].into(), p[1].into()]))
                .collect::<Vec<_>>(),
        )?
        .send(&rec_stream)?;

    // Log an extra rect to set the view bounds
    MsgSender::new("bounds")
        .with_component(&[Rect2D::XCYCWH(Vec4D([3.0, 0.0, 8.0, 6.0]))])?
        .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
