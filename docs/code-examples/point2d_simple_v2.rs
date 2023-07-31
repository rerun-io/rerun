//! Log some very simple points.

use rerun::{
    archetypes::Points2D, components::Rect2D, datatypes::Vec4D, MsgSender, RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new("points").memory()?;

    MsgSender::from_archetype("points", &Points2D::new([(0.0, 0.0), (1.0, 1.0)]))?
        .send(&rec_stream)?;

    // Log an extra rect to set the view bounds
    MsgSender::new("bounds")
        .with_component(&[Rect2D::XCYCWH(Vec4D([0.0, 0.0, 4.0, 3.0]).into())])?
        .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;

    Ok(())
}
