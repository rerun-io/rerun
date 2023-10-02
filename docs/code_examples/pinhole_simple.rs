//! Log a pinhole and a random image.

use ndarray::{Array, ShapeBuilder};
use rerun::{
    archetypes::{Image, Pinhole},
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_pinhole").memory()?;

    let mut image = Array::<u8, _>::default((3, 3, 3).f());
    image.map_inplace(|x| *x = rand::random());

    rec.log(
        "world/image",
        &Pinhole::from_focal_length_and_resolution([3., 3.], [3., 3.]),
    )?;
    rec.log("world/image", &Image::try_from(image)?)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
