//! Create and log a depth image.

use ndarray::{s, Array, ShapeBuilder};
use rerun::{
    components::{Tensor, TensorDataMeaning},
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_depth_image").memory()?;

    let mut image = Array::<u16, _>::from_elem((200, 300).f(), 65535);
    image.slice_mut(s![50..150, 50..150]).fill(20000);
    image.slice_mut(s![130..180, 100..280]).fill(45000);

    let mut tensor = Tensor::try_from(image.as_standard_layout().view())?;
    tensor.meaning = TensorDataMeaning::Depth;
    tensor.meter = Some(10000.);

    // TODO(#2792): Image archetype
    rec.log_component_lists("depth", false, 1, [&tensor as _])?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
