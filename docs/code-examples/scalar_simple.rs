//! Log a scalar over time.
use rand::{thread_rng, Rng};
use rand_distr::StandardNormal;
use rerun::components::Scalar;
use rerun::time::{TimeType, Timeline};
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new("rerun_example_scalar").memory()?;

    let step_timeline = Timeline::new("step", TimeType::Sequence);

    let mut value = 1.0;
    for step in 0..100 {
        MsgSender::new("scalar")
            .with_component(&[Scalar::from(value)])?
            .with_time(step_timeline, step)
            .send(&rec_stream)?;

        value += thread_rng().sample::<f64, _>(StandardNormal);
    }

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
