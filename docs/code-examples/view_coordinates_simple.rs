//! Log a batch of 3D arrows.

use rerun::{
    archetypes::{Arrows3D, ViewCoordinates},
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_view_coordinate").memory()?;

    rec.log("/", &ViewCoordinates::ULB)?;
    rec.log(
        "xyz",
        &Arrows3D::new(vec![[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]) // vectors
            .with_colors(vec![[255, 0, 0], [0, 255, 0], [0, 0, 255]]), // colors
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
