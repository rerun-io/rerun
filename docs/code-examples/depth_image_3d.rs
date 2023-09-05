//! Create and log a depth image.
use ndarray::{s, Array, ShapeBuilder};
use rerun::{
    components::{Pinhole, Tensor, TensorDataMeaning},
    datatypes::Mat3x3,
    external::uuid,
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_depth_image").memory()?;

    // Create a dummy depth image
    let mut image = Array::<u16, _>::from_elem((200, 300).f(), 65535);
    image.slice_mut(s![50..150, 50..150]).fill(20000);
    image.slice_mut(s![130..180, 100..280]).fill(45000);

    let mut tensor = Tensor::try_from(image.as_standard_layout().view())?;
    tensor.meaning = TensorDataMeaning::Depth;
    tensor.meter = Some(10000.);
    tensor.tensor_id = uuid::Uuid::nil().into();

    // If we log a pinhole camera model, the depth gets automatically back-projected to 3D
    // TODO(#2816): Pinhole archetype
    let (width, height) = (image.shape()[1] as f32, image.shape()[0] as f32);
    let focal_length = 200.;
    #[allow(clippy::tuple_array_conversions)]
    rec.log_component_lists(
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

    // TODO(#2792): Image archetype
    rec.log_component_lists("world/camera/depth", false, 1, [&tensor as _])?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
