//! Experimenting with multi-scalar logging.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_scalar").spawn()?;

    // Simple: Static color & stroke width.

    rec.log_static(
        "scalar",
        &rerun::SeriesLine::new()
            .with_many_color([
                rerun::Color::from_rgb(255, 0, 0),
                rerun::Color::from_rgb(0, 0, 255),
            ])
            .with_many_width([2.0, 4.0])
            .with_many_name(["sin", "cos"]),
    )?;

    for step in 0..64 {
        rec.set_time_sequence("step", step);
        rec.log(
            "scalar",
            &rerun::Scalar::update_fields()
                .with_many_scalar([(step as f64 / 10.0).sin(), (step as f64 / 10.0).cos()]),
        )?;
    }

    // Complex: Color & stroke width changing over time.

    for step in 0..64 {
        rec.set_time_sequence("step", step);

        rec.log(
            "multi_colored",
            &rerun::SeriesLine::new()
                .with_many_color([
                    rerun::Color::from_rgb(step * 4, 255 - step * 4, 0),
                    rerun::Color::from_rgb(0, step * 4, 255 - step * 4),
                ])
                .with_many_width([64.0 - step as f32, step as f32]),
        )?;
        rec.log(
            "multi_colored",
            &rerun::Scalar::update_fields().with_many_scalar([
                (step as f64 / 10.0).sin() + 2.0,
                (step as f64 / 10.0).cos() + 2.0,
            ]),
        )?;
    }

    Ok(())
}
