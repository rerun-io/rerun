//! Log some very simple points.

use rerun::{archetypes::Points2D, components::Rect2D, datatypes::Vec4D, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_points2d").memory()?;

    rec.log("points", &Points2D::new([(0.0, 0.0), (1.0, 1.0)]))?;

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
