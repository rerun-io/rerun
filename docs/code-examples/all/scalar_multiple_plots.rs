//! Log a scalar over time.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_scalar_multiple_plots").spawn()?;
    let mut lcg_state = 0_i64;

    // Set up plot styling:
    // They are logged timeless as they don't change over time and apply to all timelines.
    // Log two lines series under a shared root so that they show in the same plot by default.
    rec.log_timeless(
        "trig/sin",
        &rerun::SeriesLine::new()
            .with_color([255, 0, 0])
            .with_name("sin(0.01t)"),
    )?;
    rec.log_timeless(
        "trig/cos",
        &rerun::SeriesLine::new()
            .with_color([0, 255, 0])
            .with_name("cos(0.01t)"),
    )?;
    // Log scattered points under a different root so that they show in a different plot by default.
    rec.log_timeless("scatter/lcg", &rerun::SeriesPoint::new())?;

    for t in 0..((std::f32::consts::TAU * 2.0 * 100.0) as i64) {
        rec.set_time_sequence("step", t);

        // Log two time series under a shared root so that they show in the same plot by default.
        rec.log("trig/sin", &rerun::Scalar::new((t as f64 / 100.0).sin()))?;
        rec.log("trig/cos", &rerun::Scalar::new((t as f64 / 100.0).cos()))?;

        // Log scattered points under a different root so that it shows in a different plot by default.
        lcg_state = (1140671485_i64
            .wrapping_mul(lcg_state)
            .wrapping_add(128201163))
            % 16777216; // simple linear congruency generator
        rec.log("scatter/lcg", &rerun::Scalar::new(lcg_state as f64))?;
    }

    Ok(())
}
