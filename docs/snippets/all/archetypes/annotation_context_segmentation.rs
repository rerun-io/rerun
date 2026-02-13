//! Log a segmentation image with annotations.

use ndarray::{Array, ShapeBuilder as _, s};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_annotation_context_segmentation")
        .spawn()?;

    // create an annotation context to describe the classes
    rec.log_static(
        "segmentation",
        &rerun::AnnotationContext::new([
            (1, "red", rerun::Rgba32::from_rgb(255, 0, 0)),
            (2, "green", rerun::Rgba32::from_rgb(0, 255, 0)),
        ]),
    )?;

    // create a segmentation image
    let mut data = Array::<u8, _>::zeros((200, 300).f());
    data.slice_mut(s![50..100, 50..120]).fill(1);
    data.slice_mut(s![100..180, 130..280]).fill(2);

    rec.log(
        "segmentation/image",
        &rerun::SegmentationImage::try_from(data)?,
    )?;

    Ok(())
}
