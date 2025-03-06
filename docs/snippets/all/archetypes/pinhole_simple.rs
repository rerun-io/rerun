//! Log a pinhole and a random image.

use ndarray::{Array, ShapeBuilder as _};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_pinhole").spawn()?;

    let mut image = Array::<u8, _>::default((3, 3, 3).f());
    image.map_inplace(|x| *x = rand::random());

    rec.log(
        "world/image",
        &rerun::Pinhole::from_focal_length_and_resolution([3., 3.], [3., 3.]),
    )?;
    rec.log(
        "world/image",
        &rerun::Image::from_color_model_and_tensor(rerun::ColorModel::RGB, image)?,
    )?;

    Ok(())
}
