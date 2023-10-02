//! Log a scalar over time.

use rerun::{archetypes::TimeSeriesScalar, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_scalar").memory()?;

    for step in 0..64 {
        rec.set_time_sequence("step", step);
        rec.log("scalar", &TimeSeriesScalar::new((step as f64 / 10.0).sin()))?;
    }

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
