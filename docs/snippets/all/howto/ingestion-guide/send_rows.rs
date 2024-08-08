//! Use the send APIs to ingest scalars, one chunk at a time.

use rerun::components::Scalar;
use rerun::TimeColumn;

const NUM_STEPS: usize = 1_000_000;
const COEFF: f64 = 10.0 / NUM_STEPS as f64;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_send_columns").stdout()?;

    // Create a column containing all our `step` values.
    let steps = TimeColumn::new_sequence("step", (0..NUM_STEPS as i64).collect::<Vec<_>>());

    // Create a column containing all our scalar values.
    let scalars: Vec<Scalar> = (0..NUM_STEPS)
        .map(|step| (step as f64 * COEFF).sin())
        .map(Into::into)
        .collect();

    // Log a new chunk with all of our data, in a single call.
    //
    // NOTE: Send APIs don't inherit from the logging context: what you push is what you get.
    rec.send_columns("scalars", [steps], [&scalars as _])?;

    Ok(())
}
