//! Log some very simple rects.

use rerun::{components::Rect2D, datatypes::Vec4D, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_rect2d").memory()?;

    // TODO(#2786): Rect2D archetype
    rec.log_component_batches(
        "simple",
        false,
        1,
        [&Rect2D::XYWH(Vec4D([-1., -1., 2., 2.]).into()) as _],
    )?;

    // Log an extra rect to set the view bounds
    // TODO(#2786): Rect2D archetype
    rec.log_component_batches(
        "bounds",
        false,
        1,
        [&Rect2D::XCYCWH(Vec4D([0.0, 0.0, 4.0, 3.0]).into()) as _],
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
