//! Create and log a one dimensional tensor.
use ndarray::{Array, ShapeBuilder};
use rand::{thread_rng, Rng};
use rand_distr::StandardNormal;
use rerun::components::Tensor;
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new("rerun-example-tensors").memory()?;

    let mut data = Array::<f64, _>::default((100).f());
    data.map_inplace(|x| *x = thread_rng().sample(StandardNormal));

    MsgSender::new("tensor")
        .with_component(&[Tensor::try_from(data.as_standard_layout().view())?])?
        .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
