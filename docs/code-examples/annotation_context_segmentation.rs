//! Log a segmentation image with annotations.

use ndarray::{s, Array, ShapeBuilder};
use rerun::{
    archetypes::{AnnotationContext, SegmentationImage},
    datatypes::Color,
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) =
        RecordingStreamBuilder::new("rerun_example_annotation_context_segmentation").memory()?;

    // create an annotation context to describe the classes
    rec.log(
        "segmentation",
        &AnnotationContext::new([
            (1, "red", Color::from(0xFF0000FF)),
            (2, "green", Color::from(0x00FF00FF)),
        ]),
    )?;

    // create a segmentation image
    let mut data = Array::<u8, _>::zeros((200, 300).f());
    data.slice_mut(s![50..150, 50..120]).fill(1);
    data.slice_mut(s![100..180, 130..280]).fill(2);

    rec.log("segmentation/image", &SegmentationImage::try_from(data)?)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
