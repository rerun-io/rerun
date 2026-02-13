//! Update a transform over time, in a single operation.
//!
//! This is semantically equivalent to the `transform3d_row_updates` example, albeit much faster.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_transform3d_column_updates").spawn()?;

    rec.set_time_sequence("tick", 0);
    rec.log(
        "box",
        &[
            &rerun::Boxes3D::from_half_sizes([(4.0, 2.0, 1.0)])
                .with_fill_mode(rerun::FillMode::Solid) as &dyn rerun::AsComponents,
            &rerun::TransformAxes3D::new(10.0),
        ],
    )?;

    let translations = (0..100).map(|t| [0.0, 0.0, t as f32 / 10.0]);
    let rotations = (0..100)
        .map(|t| truncated_radians((t * 4) as f32))
        .map(|rad| rerun::RotationAxisAngle::new([0.0, 1.0, 0.0], rerun::Angle::from_radians(rad)));

    let ticks = rerun::TimeColumn::new_sequence("tick", 1..101);
    rec.send_columns(
        "box",
        [ticks],
        rerun::Transform3D::default()
            .with_many_translation(translations)
            .with_many_rotation_axis_angle(rotations)
            .columns_of_unit_batches()?,
    )?;

    Ok(())
}

fn truncated_radians(deg: f32) -> f32 {
    ((deg.to_radians() * 1000.0) as i32) as f32 / 1000.0
}
