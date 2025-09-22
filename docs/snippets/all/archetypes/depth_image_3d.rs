//! Create and log a depth image.
use ndarray::{Array, ShapeBuilder as _, s};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_depth_image_3d").spawn()?;

    let width = 300;
    let height = 200;
    let mut image = Array::<u16, _>::from_elem((height, width).f(), 65535);
    image.slice_mut(s![50..150, 50..150]).fill(20000);
    image.slice_mut(s![130..180, 100..280]).fill(45000);

    let depth_image = rerun::DepthImage::try_from(image)?
        .with_meter(10000.0)
        .with_colormap(rerun::components::Colormap::Viridis);

    // If we log a pinhole camera model, the depth gets automatically back-projected to 3D
    rec.log(
        "world/camera",
        &rerun::Pinhole::from_focal_length_and_resolution(
            [200.0, 200.0],
            [width as f32, height as f32],
        ),
    )?;

    rec.log("world/camera/depth", &depth_image)?;

    Ok(())
}
