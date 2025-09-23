//! Create and log a segmentation image.

use ndarray::{Array, ShapeBuilder as _, s};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_segmentation_image").spawn()?;

    // create a segmentation image
    let mut image = Array::<u8, _>::zeros((8, 12).f());
    image.slice_mut(s![0..4, 0..6]).fill(1);
    image.slice_mut(s![4..8, 6..12]).fill(2);

    // create an annotation context to describe the classes
    let annotation = rerun::AnnotationContext::new([
        (1, "red", rerun::Rgba32::from_rgb(255, 0, 0)),
        (2, "green", rerun::Rgba32::from_rgb(0, 255, 0)),
    ]);

    // log the annotation and the image
    rec.log_static("/", &annotation)?;

    rec.log("image", &rerun::SegmentationImage::try_from(image)?)?;

    Ok(())
}
