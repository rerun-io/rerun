//! Log a simple line strip.

use rerun::{
    archetypes::LineStrips2D, components::Rect2D, datatypes::Vec4D, MsgSender,
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_line_strip2d").memory()?;

    let points = [[0., 0.], [2., 1.], [4., -1.], [6., 0.]];
    MsgSender::from_archetype("strip", &LineStrips2D::new([points]))?.send(&rec)?;

    // Log an extra rect to set the view bounds
    MsgSender::new("bounds")
        .with_component(&[Rect2D::XCYCWH(Vec4D([3.0, 0.0, 8.0, 6.0]).into())])?
        .send(&rec)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
