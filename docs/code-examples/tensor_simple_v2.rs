//! Create and log a tensor.
use ndarray::{Array, ShapeBuilder};
use rerun::archetypes::Tensor;
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new(env!("CARGO_BIN_NAME")).memory()?;

    let mut data = Array::<u8, _>::default((8, 6, 3, 5).f());
    data.map_inplace(|x| *x = rand::random());

    let tensor = Tensor::try_from(data)?.with_names(["batch", "channel", "height", "width"]);

    MsgSender::from_archetype("tensor", &tensor)?.send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
