//! Log a segmentation image with annotations.

use ndarray::{s, Array, ShapeBuilder};
use rerun::{
    archetypes::AnnotationContext,
    components::{Tensor, TensorDataMeaning},
    datatypes::Color,
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) =
        RecordingStreamBuilder::new("rerun_example_annotation_context_segmentation").memory()?;

    // create a segmentation image
    let mut data = Array::<u8, _>::zeros((200, 300).f());
    data.slice_mut(s![50..100, 50..120]).fill(1);
    data.slice_mut(s![100..180, 130..280]).fill(2);

    let mut image = Tensor::try_from(data.as_standard_layout().view())?;
    image.meaning = TensorDataMeaning::ClassId;

    // create an annotation context to describe the classes
    rec.log(
        "segmentation",
        &AnnotationContext::new([
            (1, "red", Color::from(0xFF0000FF)),
            (2, "green", Color::from(0x00FF00FF)),
        ]),
    )?;

    // TODO(#2792): SegmentationImage archetype
    rec.log_component_lists("segmentation/image", false, 1, [&image as _])?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
