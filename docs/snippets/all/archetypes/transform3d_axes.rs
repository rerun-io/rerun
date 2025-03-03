//! Log different transforms with visualized coordinates axes.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_transform3d_axes").spawn()?;

    rec.set_index("step", sequence=0);

    rec.log(
        "base",
        &rerun::Transform3D::clear_fields().with_axis_length(1.0),
    )?;

    for deg in 0..360 {
        rec.set_index("step", sequence=deg);
        rec.log(
            "base/rotated",
            &rerun::Transform3D::clear_fields()
                .with_axis_length(0.5)
                .with_rotation(rerun::RotationAxisAngle::new(
                    [1.0, 1.0, 1.0],
                    rerun::Angle::from_degrees(deg as f32),
                )),
        )?;
        rec.log(
            "base/rotated/translated",
            &rerun::Transform3D::clear_fields()
                .with_axis_length(0.5)
                .with_translation([2.0, 0.0, 0.0]),
        )?;
    }

    Ok(())
}
