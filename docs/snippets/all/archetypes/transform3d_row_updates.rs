//! Update a transform over time.
//!
//! See also the `transform3d_column_updates` example, which achieves the same thing in a single operation.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_transform3d_row_updates").spawn()?;

    rec.set_time_sequence("tick", 0);
    rec.log(
        "box",
        &[
            &rerun::Boxes3D::from_half_sizes([(4.0, 2.0, 1.0)])
                .with_fill_mode(rerun::FillMode::Solid) as &dyn rerun::AsComponents,
            &rerun::TransformAxes3D::new(10.0),
        ],
    )?;

    for t in 0..100 {
        rec.set_time_sequence("tick", t + 1);
        rec.log(
            "box",
            &rerun::Transform3D::default()
                .with_translation([0.0, 0.0, t as f32 / 10.0])
                .with_rotation(rerun::RotationAxisAngle::new(
                    [0.0, 1.0, 0.0],
                    rerun::Angle::from_radians(truncated_radians((t * 4) as f32)),
                )),
        )?;
    }

    Ok(())
}

fn truncated_radians(deg: f32) -> f32 {
    ((deg.to_radians() * 1000.0) as i32) as f32 / 1000.0
}
