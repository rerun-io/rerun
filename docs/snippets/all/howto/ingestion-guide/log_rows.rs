//! Use the log APIs to log scalars over time.

const NUM_STEPS: usize = 100_000;
const COEFF: f64 = 10.0 / NUM_STEPS as f64;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_log_rows").stdout()?;

    for step in 0..NUM_STEPS {
        // Set the `step` timeline in the logging context to the current time.
        rec.set_time_sequence("step", step as i64);

        // Log a new row containing a single scalar.
        // This will inherit from the logging context, and thus be logged at the current `step`.
        rec.log("scalars", &rerun::Scalar::new((step as f64 * COEFF).sin()))?;
    }

    Ok(())
}
