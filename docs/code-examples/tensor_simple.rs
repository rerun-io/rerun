//! Create and log a tensor.
use ndarray::{Array, ShapeBuilder};
use rerun::components::Tensor;
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_tensors").memory()?;

    let mut data = Array::<u8, _>::default((8, 6, 3, 5).f());
    data.map_inplace(|x| *x = rand::random());

    let mut tensor = Tensor::try_from(data.as_standard_layout().view())?;
    tensor.shape[0].name = Some("width".to_owned());
    tensor.shape[1].name = Some("height".to_owned());
    tensor.shape[2].name = Some("channel".to_owned());
    tensor.shape[3].name = Some("batch".to_owned());

    MsgSender::new("tensor")
        .with_component(&[tensor])?
        .send(&rec)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
