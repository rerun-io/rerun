//! Create and log a one dimensional tensor.

use ndarray::{Array, ShapeBuilder};
use rand::{thread_rng, Rng};
use rand_distr::StandardNormal;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = rerun::RecordingStreamBuilder::new("rerun_example_tensors").memory()?;

    let mut data = Array::<f64, _>::default((100).f());
    data.map_inplace(|x| *x = thread_rng().sample(StandardNormal));

    rec.log(
        "tensor",
        &rerun::Tensor::try_from(data.as_standard_layout().view())?,
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
