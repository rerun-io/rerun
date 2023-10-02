//! Create and log a segmentation image.

use ndarray::{s, Array, ShapeBuilder};
use rerun::datatypes::Color;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) =
        rerun::RecordingStreamBuilder::new("rerun_example_segmentation_image").memory()?;

    // create a segmentation image
    let mut image = Array::<u8, _>::zeros((8, 12).f());
    image.slice_mut(s![0..4, 0..6]).fill(1);
    image.slice_mut(s![4..8, 6..12]).fill(2);

    // create an annotation context to describe the classes
    let annotation = rerun::AnnotationContext::new([
        (1, "red", Color::from(0xFF0000FF)),
        (2, "green", Color::from(0x00FF00FF)),
    ]);

    // log the annotation and the image
    rec.log("/", &annotation)?;

    rec.log("image", &rerun::SegmentationImage::try_from(image)?)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
