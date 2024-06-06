//! Log different transforms.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_transform3d_axes").spawn()?;

    let base_axes = rerun::Axes3D::new().with_length(1.0);
    let other_axes = rerun::Axes3D::new().with_length(0.5);

    rec.log_static("base", &base_axes)?;
    rec.log_static("base/rotated", &other_axes)?;
    rec.log_static("base/rotated/translated", &other_axes)?;

    for deg in 0..360 {
        rec.set_time_sequence("step", deg);
        rec.log(
            "base/rotated",
            &rerun::Transform3D::from_rotation(rerun::RotationAxisAngle::new(
                [1.0, 1.0, 1.0],
                rerun::Angle::Degrees(deg as f32),
            )),
        )?;
        rec.log(
            "base/rotated/translated",
            &rerun::Transform3D::from_translation([2.0, 0.0, 0.0]),
        )?;
    }

    Ok(())
}
