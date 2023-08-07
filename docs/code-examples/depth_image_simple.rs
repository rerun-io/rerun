//! Create and log a depth image.
use ndarray::{s, Array, ShapeBuilder};
use rerun::components::{Tensor, TensorDataMeaning};
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new(env!("CARGO_BIN_NAME")).memory()?;

    let mut image = Array::<u16, _>::from_elem((200, 300).f(), 65535);
    image.slice_mut(s![50..150, 50..150]).fill(20000);
    image.slice_mut(s![130..180, 100..280]).fill(45000);

    let mut tensor = Tensor::try_from(image.as_standard_layout().view())?;
    tensor.meaning = TensorDataMeaning::Depth;

    MsgSender::new("depth")
        .with_component(&[tensor])?
        .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
