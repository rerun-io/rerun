//! Log different transforms with visualized coordinates axes.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_transform3d_axes").spawn()?;

    let mut step = 0;

    rec.set_time_sequence("step", step);
    rec.log(
        "box",
        &[
            &rerun::Boxes3D::from_half_sizes([(4.0, 2.0, 1.0)])
                .with_fill_mode(rerun::FillMode::Solid) as &dyn rerun::AsComponents,
            &rerun::Transform3D::default().with_axis_length(10.0),
        ],
    )?;

    for deg in 0..=45 {
        step += 1;
        rec.set_time_sequence("step", step);

        let rad = truncated_radians((deg * 4) as f32);
        rec.log(
            "box",
            &rerun::Transform3D::update_fields().with_rotation(rerun::RotationAxisAngle::new(
                [0.0, 1.0, 0.0],
                rerun::Angle::from_radians(rad),
            )),
        )?;
    }

    for t in 0..=50 {
        step += 1;
        rec.set_time_sequence("step", step);
        rec.log(
            "box",
            &rerun::Transform3D::update_fields().with_translation([0.0, 0.0, t as f32 / 10.0]),
        )?;
    }

    for deg in 0..=45 {
        step += 1;
        rec.set_time_sequence("step", step);

        let rad = truncated_radians(((deg + 45) * 4) as f32);
        rec.log(
            "box",
            &rerun::Transform3D::update_fields().with_rotation(rerun::RotationAxisAngle::new(
                [0.0, 1.0, 0.0],
                rerun::Angle::from_radians(rad),
            )),
        )?;
    }

    step += 1;
    rec.set_time_sequence("step", step);
    rec.log(
        "box",
        &rerun::Transform3D::clear_fields().with_axis_length(15.0),
    )?;

    Ok(())
}

fn truncated_radians(deg: f32) -> f32 {
    ((deg.to_radians() * 1000.0) as i32) as f32 / 1000.0
}
