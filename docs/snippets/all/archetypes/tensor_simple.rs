//! Create and log a tensor.

use ndarray::{Array, ShapeBuilder as _};
use rand::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_tensor").spawn()?;

    let mut data = Array::<u8, _>::default((8, 6, 3, 5).f());
    let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
    data.map_inplace(|x| *x = rng.random());

    let tensor =
        rerun::Tensor::try_from(data)?.with_dim_names(["width", "height", "channel", "batch"]);
    rec.log("tensor", &tensor)?;

    Ok(())
}
