//! Very minimal test of using the send columns APIs.

use re_chunk::TimeColumn;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_send_columns").spawn()?;

    let timeline_values = TimeColumn::new_sequence("step", 0..64);
    let scalar_data = (0..64).map(|step| (step as f64 / 10.0).sin());

    rec.send_columns(
        "scalar",
        [timeline_values],
        rerun::Scalars::update_fields()
            .with_scalars(scalar_data)
            .columns_of_unit_batches()?,
    )?;

    Ok(())
}
