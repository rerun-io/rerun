//! Use the `send_columns` API to send scalars over time in a single call.

use rerun::TimeColumn;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_scalar_send_columns").spawn()?;

    const STEPS: i64 = 64;

    let times = TimeColumn::new_sequence("step", 0..STEPS);
    let scalars = (0..STEPS).map(|step| (step as f64 / 10.0).sin());

    rec.send_columns(
        "scalars",
        [times],
        rerun::Scalar::update_fields()
            .with_many_scalar(scalars)
            .columns_of_unit_batches()?,
    )?;

    Ok(())
}
