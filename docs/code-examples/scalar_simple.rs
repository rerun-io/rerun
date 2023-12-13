//! Log a scalar over time.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_scalar").save("/tmp/kek.rrd")?;

    for step in 0..1_000_000 {
        rec.set_time_sequence("step", step);
        rec.log(
            "scalar",
            &rerun::TimeSeriesScalar::new((step as f64 / 10.0).sin()),
        )?;
    }

    Ok(())
}
