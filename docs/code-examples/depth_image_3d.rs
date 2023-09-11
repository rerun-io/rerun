//! Create and log a depth image.
use ndarray::{s, Array, ShapeBuilder};
use rerun::{
    archetypes::DepthImage, components::Pinhole, datatypes::Mat3x3, RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_depth_image").memory()?;

    // Create a dummy depth image
    let mut image = Array::<u16, _>::from_elem((8, 12).f(), 65535);
    image.slice_mut(s![0..4, 0..6]).fill(20000);
    image.slice_mut(s![4..8, 6..12]).fill(45000);

    let depth_image = DepthImage::try_from(image.clone())?.with_meter(10000.0);

    // If we log a pinhole camera model, the depth gets automatically back-projected to 3D
    // TODO(#2816): Pinhole archetype
    let (width, height) = (image.shape()[1] as f32, image.shape()[0] as f32);
    let focal_length = 200.;
    #[allow(clippy::tuple_array_conversions)]
    rec.log_component_batches(
        "world/camera",
        false,
        1,
        [&Pinhole {
            // NOTE: Column major constructor!
            #[rustfmt::skip]
            image_from_cam: Mat3x3::from([
                [ focal_length, 0.0,          0.0, ],
                [ 0.0,          focal_length, 0.0, ],
                [ width / 2.0,  height / 2.0, 1.0, ],
            ])
            .into(),
            resolution: Some([width, height].into()),
        } as _],
    )?;

    rec.log("world/camera/depth", &depth_image)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
