//! Log a scalar over time.

use rand::{thread_rng, Rng};
use rand_distr::StandardNormal;
use rerun::{components::Scalar, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_scalar").memory()?;

    let mut value = 1.0;
    for step in 0..100 {
        rec.set_time_sequence("step", step);
        rec.log_component_lists("scalar", false, 1, [&Scalar::from(value) as _])?;
        value += thread_rng().sample::<f64, _>(StandardNormal);
    }

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
