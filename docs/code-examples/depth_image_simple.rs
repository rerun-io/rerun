//! Create and log a depth image.

use ndarray::{s, Array, ShapeBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) =
        rerun::RecordingStreamBuilder::new("rerun_example_depth_image").memory()?;

    let mut image = Array::<u16, _>::from_elem((200, 300).f(), 65535);
    image.slice_mut(s![50..150, 50..150]).fill(20000);
    image.slice_mut(s![130..180, 100..280]).fill(45000);

    let depth_image = rerun::DepthImage::try_from(image)?.with_meter(10_000.0);

    rec.log("depth", &depth_image)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
