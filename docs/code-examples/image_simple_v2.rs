//! Create and log an image
use ndarray::{s, Array, ShapeBuilder};
use rerun::archetypes::Image;
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new(env!("CARGO_BIN_NAME")).memory()?;

    let mut image = Array::<u8, _>::zeros((200, 300, 3).f());
    image.slice_mut(s![.., .., 0]).fill(255);
    image.slice_mut(s![50..150, 50..150, 0]).fill(0);
    image.slice_mut(s![50..150, 50..150, 1]).fill(255);

    let image = Image::try_from(image)?;

    MsgSender::from_archetype("image", &image)?.send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
