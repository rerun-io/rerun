//! Create and log a depth image.

use ndarray::{s, Array, ShapeBuilder};
use rerun::{archetypes::DepthImage, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_depth_image").memory()?;

    let mut image = Array::<u16, _>::from_elem((8, 12).f(), 65535);
    image.slice_mut(s![0..4, 0..6]).fill(20000);
    image.slice_mut(s![4..8, 6..12]).fill(45000);

    let depth_image = DepthImage::try_from(image)?;

    rec.log("depth", &depth_image)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
