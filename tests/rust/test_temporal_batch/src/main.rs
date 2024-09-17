//! Very minimal test of using the send columns APIs.

use re_chunk::TimeColumn;
use rerun::components::Scalar;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_send_columns").spawn()?;

    // Native time / scalars
    let timeline_values = (0..64).collect::<Vec<_>>();

    let scalar_data: Vec<f64> = timeline_values
        .iter()
        .map(|step| (*step as f64 / 10.0).sin())
        .collect();

    // Convert to rerun time / scalars
    let timeline_values = TimeColumn::new_sequence("step", timeline_values);
    let scalar_data: Vec<Scalar> = scalar_data.into_iter().map(Into::into).collect();

    rec.send_columns("scalar", [timeline_values], [&scalar_data as _])?;

    Ok(())
}
