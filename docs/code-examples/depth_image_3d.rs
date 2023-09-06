//! Create and log a depth image.
use ndarray::{s, Array, ShapeBuilder};
use rerun::{
    archetypes::DepthImage,
    components::Pinhole,
    datatypes::{Mat3x3, Vec2D},
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_depth_image").memory()?;

    // Create a dummy depth image
    let mut image = Array::<u16, _>::from_elem((200, 300).f(), 65535);
    image.slice_mut(s![50..150, 50..150]).fill(20000);
    image.slice_mut(s![130..180, 100..280]).fill(45000);

    let mut depth_image =
        DepthImage::try_from(image.as_standard_layout().view())?.with_meter(10000);

    // If we log a pinhole camera model, the depth gets automatically back-projected to 3D
    // TODO(#2816): Pinhole archetype
    let focal_length = 200.;
    rec.log_component_lists(
        "world/camera",
        false,
        1,
        [&Pinhole {
            image_from_cam: Mat3x3::from([
                [focal_length, 0., image.shape()[1] as f32 / 2.],
                [0., focal_length, image.shape()[0] as f32 / 2.],
                [0., 0., 1.],
            ])
            .into(),
            resolution: Some(
                Vec2D::from([image.shape()[1] as f32, image.shape()[0] as f32]).into(),
            ),
        } as _],
    )?;

    rec.log("world/camera/depth", &depth_image);

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
