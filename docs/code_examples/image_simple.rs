//! Create and log an image

use ndarray::{s, Array, ShapeBuilder};
use rerun::{archetypes::Image, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_image_simple").memory()?;

    let mut image = Array::<u8, _>::zeros((8, 12, 3).f());
    image.slice_mut(s![.., .., 0]).fill(255);
    image.slice_mut(s![0..4, 0..6, 0]).fill(0);
    image.slice_mut(s![0..4, 0..6, 1]).fill(255);

    rec.log("image", &Image::try_from(image)?)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
