//! Create and log a depth image.
use ndarray::{s, Array, ShapeBuilder};
use rerun::{
    archetypes::{DepthImage, Pinhole},
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_depth_image").memory()?;

    // Create a dummy depth image
    let mut image = Array::<u16, _>::from_elem((8, 12).f(), 65535);
    image.slice_mut(s![0..4, 0..6]).fill(20000);
    image.slice_mut(s![4..8, 6..12]).fill(45000);

    let depth_image = DepthImage::try_from(image.clone())?.with_meter(10000.0);

    // If we log a pinhole camera model, the depth gets automatically back-projected to 3D
    rec.log(
        "world/camera",
        &Pinhole::from_focal_length_and_resolution(
            [200.0, 200.0],
            [image.shape()[1] as f32, image.shape()[0] as f32],
        ),
    )?;

    rec.log("world/camera/depth", &depth_image)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
