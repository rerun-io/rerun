//! Log a scalar over time.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_scalar").spawn()?;

    // Set up plot styling: Logged timeless since it never changes and affects all timelines.
    rec.log_timeless("scalar", &rerun::SeriesPoint::new())?;

    // Log the data on a timeline called "step".
    for step in 0..64 {
        rec.set_time_sequence("step", step);
        rec.log("scalar", &rerun::Scalar::new((step as f64 / 10.0).sin()))?;
    }

    Ok(())
}
