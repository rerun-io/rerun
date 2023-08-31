//! Log some very simple points.

use rerun::{
    archetypes::Points2D, components::Rect2D, datatypes::Vec4D, MsgSender, RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_points2d").memory()?;

    MsgSender::from_archetype("points", &Points2D::new([(0.0, 0.0), (1.0, 1.0)]))?.send(&rec)?;

    // Log an extra rect to set the view bounds
    MsgSender::new("bounds")
        .with_component(&[Rect2D::XCYCWH(Vec4D([0.0, 0.0, 4.0, 3.0]).into())])?
        .send(&rec)?;

    rerun::native_viewer::show(storage.take())?;

    Ok(())
}
