//! Log rectangles with different colors and labels using annotation context

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_annotation_context_rects").spawn()?;

    // Log an annotation context to assign a label and color to each class
    rec.log_static(
        "/",
        &rerun::AnnotationContext::new([
            (1, "red", rerun::Rgba32::from_rgb(255, 0, 0)),
            (2, "green", rerun::Rgba32::from_rgb(0, 255, 0)),
        ]),
    )?;

    // Log a batch of 2 rectangles with different class IDs
    rec.log(
        "detections",
        &rerun::Boxes2D::from_mins_and_sizes([(-2., -2.), (0., 0.)], [(3., 3.), (2., 2.)])
            .with_class_ids([1, 2]),
    )?;

    // Log an extra rect to set the view bounds
    rec.log("bounds", &rerun::Boxes2D::from_half_sizes([(2.5, 2.5)]))?;

    Ok(())
}
