//! Log a scalar over time.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_scalar_multiple_plots").spawn()?;
    let mut lcg_state = 0_i64;

    for t in 0..((std::f32::consts::TAU * 2.0 * 100.0) as i64) {
        rec.set_time_sequence("step", t);

        // Log two time series under a shared root so that they show in the same plot by default.
        rec.log(
            "trig/sin",
            &rerun::TimeSeriesScalar::new((t as f64 / 100.0).sin())
                .with_label("sin(0.01t)")
                .with_color([255, 0, 0]),
        )?;
        rec.log(
            "trig/cos",
            &rerun::TimeSeriesScalar::new((t as f64 / 100.0).cos())
                .with_label("cos(0.01t)")
                .with_color([0, 255, 0]),
        )?;

        // Log scattered points under a different root so that it shows in a different plot by default.
        lcg_state = (1140671485_i64
            .wrapping_mul(lcg_state)
            .wrapping_add(128201163))
            % 16777216; // simple linear congruency generator
        rec.log(
            "scatter/lcg",
            &rerun::TimeSeriesScalar::new(lcg_state as f64).with_scattered(true),
        )?;
    }

    Ok(())
}
