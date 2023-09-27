//! Change the view coordinates for the scene.
use rerun::{
    archetypes::{Arrows3D, ViewCoordinates},
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_view_coordinates").memory()?;

    rec.log_timeless("world", &ViewCoordinates::RIGHT_HAND_Z_UP)?; // Set an up-axis
    rec.log(
        "world/xyz",
        &Arrows3D::from_vectors(
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], //
        )
        .with_colors([[255, 0, 0], [0, 255, 0], [0, 0, 255]]),
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
