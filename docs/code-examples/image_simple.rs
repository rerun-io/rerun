//! Create and log an image

use ndarray::{s, Array, ShapeBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_image_simple").spawn()?;

    let mut image = Array::<u8, _>::zeros((200, 300, 3).f());
    image.slice_mut(s![.., .., 0]).fill(255);
    image.slice_mut(s![50..150, 50..150, 0]).fill(0);
    image.slice_mut(s![50..150, 50..150, 1]).fill(255);

    rec.log("image", &rerun::Image::try_from(image)?)?;

    Ok(())
}
