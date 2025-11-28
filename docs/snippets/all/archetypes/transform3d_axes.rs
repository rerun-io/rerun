//! Log different transforms with visualized coordinates axes.

use rerun::AsComponents;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_transform3d_axes").spawn()?;

    rec.set_time_sequence("step", 0);

    rec.log(
        "base",
        &[
            &rerun::Transform3D::new() as &dyn AsComponents,
            &rerun::TransformAxes3D::new(1.0),
        ],
    )?;

    for deg in 0..360 {
        rec.set_time_sequence("step", deg);
        rec.log(
            "base/rotated",
            &[
                &rerun::Transform3D::new().with_rotation(rerun::RotationAxisAngle::new(
                    [1.0, 1.0, 1.0],
                    rerun::Angle::from_degrees(deg as f32),
                )) as &dyn AsComponents,
                &rerun::TransformAxes3D::new(0.5),
            ],
        )?;
        rec.log(
            "base/rotated/translated",
            &[
                &rerun::Transform3D::new().with_translation([2.0, 0.0, 0.0]) as &dyn AsComponents,
                &rerun::TransformAxes3D::new(0.5),
            ],
        )?;
    }

    Ok(())
}
