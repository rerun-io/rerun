//! Log rectangles with different colors and labels using annotation context

use rerun::{
    archetypes::AnnotationContext,
    components::{ClassId, Rect2D},
    datatypes::{Color, Vec4D},
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
    // TODO(#2786): Rect2D archetype
    rec.log_component_lists(
        "detections",
        false,
        2,
        [
            &[
                Rect2D::XYWH(Vec4D([-2., -2., 3., 3.]).into()),
                Rect2D::XYWH(Vec4D([0., 0., 2., 2.]).into()),
            ] as _,
            &[ClassId::from(1), ClassId::from(2)] as _,
        ],
    )?;

    // Log an extra rect to set the view bounds
    // TODO(#2786): Rect2D archetype
    rec.log_component_lists(
        "bounds",
        false,
        1,
        [&[Rect2D::XCYCWH(Vec4D([0.0, 0.0, 5.0, 5.0]).into())] as _],
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
