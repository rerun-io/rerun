//! Log a pinhole and a random image.

use ndarray::{Array, ShapeBuilder};
use rerun::{
    components::{Pinhole, Tensor},
    datatypes::{Mat3x3, Vec2D},
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_pinhole").memory()?;

    let mut image = Array::<u8, _>::default((3, 3, 3).f());
    image.map_inplace(|x| *x = rand::random());

    // TODO(#2816): Pinhole archetype
    rec.log_component_lists(
        "world/image",
        false,
        1,
        [
            &Pinhole {
                image_from_cam: Mat3x3::from([[3., 0., 1.5], [0., 3., 1.5], [0., 0., 1.]]).into(),
                resolution: Some(Vec2D::from([3., 3.]).into()),
            } as _,
            &Tensor::try_from(image.as_standard_layout().view())? as _,
        ],
    )?;
    rec.log("world/image", &Image::try_from(image));

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
