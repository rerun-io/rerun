//! Create and log an image
use ndarray::{s, Array, ShapeBuilder};
use rerun::components::Tensor;
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_images").memory()?;

    let mut image = Array::<u8, _>::zeros((200, 300, 3).f());
    image.slice_mut(s![.., .., 0]).fill(255);
    image.slice_mut(s![50..150, 50..150, 0]).fill(0);
    image.slice_mut(s![50..150, 50..150, 1]).fill(255);

    MsgSender::new("image")
        .with_component(&[Tensor::try_from(image.as_standard_layout().view())?])?
        .send(&rec)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
