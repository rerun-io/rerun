//! Log a segmentation image with annotations.

use ndarray::{s, Array, ShapeBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) =
        rerun::RecordingStreamBuilder::new("rerun_example_annotation_context_segmentation")
            .memory()?;

    // create an annotation context to describe the classes
    rec.log_timeless(
        "segmentation",
        &rerun::AnnotationContext::new([
            (1, "red", rerun::Rgba32::from(0xFF0000FF)),
            (2, "green", rerun::Rgba32::from(0x00FF00FF)),
        ]),
    )?;

    // create a segmentation image
    let mut data = Array::<u8, _>::zeros((8, 12).f());
    data.slice_mut(s![0..4, 0..6]).fill(1);
    data.slice_mut(s![4..8, 6..12]).fill(2);

    rec.log(
        "segmentation/image",
        &rerun::SegmentationImage::try_from(data)?,
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
