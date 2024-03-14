//! Log a scalar over time.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_series_line_style").spawn()?;

    // Set up plot styling:
    // They are logged timeless as they don't change over time and apply to all timelines.
    // Log two lines series under a shared root so that they show in the same plot by default.
    rec.log_timeless(
        "trig/sin",
        &rerun::SeriesLine::new()
            .with_color([255, 0, 0])
            .with_name("sin(0.01t)")
            .with_width(2.0),
    )?;
    rec.log_timeless(
        "trig/cos",
        &rerun::SeriesLine::new()
            .with_color([0, 255, 0])
            .with_name("cos(0.01t)")
            .with_width(4.0),
    )?;

    for t in 0..((std::f32::consts::TAU * 2.0 * 100.0) as i64) {
        rec.set_time_sequence("step", t);

        // Log two time series under a shared root so that they show in the same plot by default.
        rec.log("trig/sin", &rerun::Scalar::new((t as f64 / 100.0).sin()))?;
        rec.log("trig/cos", &rerun::Scalar::new((t as f64 / 100.0).cos()))?;
    }

    Ok(())
}
