//! Log a scalar over time.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_scalar")
        .spawn(rerun::default_flush_timeout())?;

    for step in 0..64 {
        rec.set_time_sequence("step", step);
        rec.log(
            "scalar",
            &rerun::TimeSeriesScalar::new((step as f64 / 10.0).sin()),
        )?;
    }

    Ok(())
}
