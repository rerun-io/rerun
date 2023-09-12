//! Log rectangles with different colors and labels using annotation context

use rerun::{
    archetypes::{AnnotationContext, Boxes2D},
    datatypes::Color,
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) =
        RecordingStreamBuilder::new("rerun_example_annotation_context_rects").memory()?;

    // Log an annotation context to assign a label and color to each class
    rec.log(
        "/",
        &AnnotationContext::new([
            (1, "red", Color::from(0xFF0000FF)),
            (2, "green", Color::from(0x00FF00FF)),
        ]),
    )?;

    // Log a batch of 2 rectangles with different class IDs
    rec.log(
        "detections",
        &Boxes2D::from_mins_and_sizes([(-2., -2.), (0., 0.)], [(3.0, 3.0), (2.0, 2.0)])
            .with_class_ids([1, 2]),
    )?;

    // Log an extra rect to set the view bounds
    rec.log(
        "bounds",
        &Boxes2D::new([(2.5, 2.5)]).with_centers([(2.5, 2.5)]),
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
