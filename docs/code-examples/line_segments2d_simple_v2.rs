//! Log a couple 2D line segments using 2D line strips.

use rerun::{
    archetypes::LineStrips2D, components::Rect2D, datatypes::Vec4D, MsgSender,
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) =
        RecordingStreamBuilder::new("rerun-example-line_segments2d").memory()?;

    let points = [[0., 0.], [2., 1.], [4., -1.], [6., 0.]];
    MsgSender::from_archetype("segments", &LineStrips2D::new(points.chunks(2)))?
        .send(&rec_stream)?;

    // Log an extra rect to set the view bounds
    MsgSender::new("bounds")
        .with_component(&[Rect2D::XCYCWH(Vec4D([3.0, 0.0, 8.0, 6.0]).into())])?
        .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
