//! Very minimal test of using the temporal batch APIs.

use rerun::components::Scalar;
use rerun::ChunkTimeline;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_temporal_batch").spawn()?;

    // Native time / scalars
    let timeline_values = (0..64).collect::<Vec<_>>();

    let scalar_data: Vec<f64> = timeline_values
        .iter()
        .map(|step| (*step as f64 / 10.0).sin())
        .collect();

    // Convert to rerun time / scalars
    let timeline_values = ChunkTimeline::new_sequence("step", timeline_values);
    let scalar_data = scalar_data.into_iter().map(Scalar).collect::<Vec<_>>();

    rec.log_temporal_batch("scalars", [timeline_values], [&scalar_data as _])?;

    Ok(())
}
