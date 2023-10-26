//! Create and log a tensor.

use ndarray::{Array, ShapeBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_tensor_simple")
        .spawn(rerun::default_flush_timeout())?;

    let mut data = Array::<u8, _>::default((8, 6, 3, 5).f());
    data.map_inplace(|x| *x = rand::random());

    let tensor =
        rerun::Tensor::try_from(data)?.with_dim_names(["batch", "channel", "height", "width"]);
    rec.log("tensor", &tensor)?;

    Ok(())
}
