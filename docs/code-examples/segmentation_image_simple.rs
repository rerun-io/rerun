//! Create and log a segmentation image.

use ndarray::{s, Array, ShapeBuilder};
use rerun::{
    archetypes::AnnotationContext,
    components::{Tensor, TensorDataMeaning},
    datatypes::Color,
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) =
        RecordingStreamBuilder::new("rerun_example_segmentation_image").memory()?;

    // create a segmentation image
    let mut image = Array::<u8, _>::zeros((200, 300).f());
    image.slice_mut(s![50..100, 50..120]).fill(1);
    image.slice_mut(s![100..180, 130..280]).fill(2);

    // TODO(#2792): SegmentationImage archetype
    let mut tensor = Tensor::try_from(image.as_standard_layout().view())?;
    tensor.meaning = TensorDataMeaning::ClassId;

    // create an annotation context to describe the classes
    let annotation = AnnotationContext::new([
        (1, "red", Color::from(0xFF0000FF)),
        (2, "green", Color::from(0x00FF00FF)),
    ]);

    // log the annotation and the image
    rec.log("/", &annotation)?;

    // TODO(#2792): SegmentationImage archetype
    rec.log_component_lists("image", false, 1, [&tensor as _])?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
