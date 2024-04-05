//! Change the view coordinates for the scene.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_view_coordinates").spawn()?;

    rec.log_static("world", &rerun::ViewCoordinates::RIGHT_HAND_Z_UP)?; // Set an up-axis
    rec.log(
        "world/xyz",
        &rerun::Arrows3D::from_vectors(
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], //
        )
        .with_colors([[255, 0, 0], [0, 255, 0], [0, 0, 255]]),
    )?;

    Ok(())
}
