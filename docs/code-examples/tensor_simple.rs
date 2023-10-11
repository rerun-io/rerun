//! Create and log a tensor.

use ndarray::{Array, ShapeBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) =
        rerun::RecordingStreamBuilder::new("rerun_example_tensor_simple").memory()?;

    let mut data = Array::<u8, _>::default((8, 6, 3, 5).f());
    data.map_inplace(|x| *x = rand::random());

    let tensor = rerun::Tensor::try_from(data)?.with_names(["batch", "channel", "height", "width"]);
    rec.log("tensor", &tensor)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
