//! Log rectangles with different colors and labels using annotation context
use rerun::{
    archetypes::AnnotationContext,
    components::{ClassId, Rect2D},
    datatypes::{AnnotationInfo, Color, Label, Vec4D},
    MsgSender, RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) =
        RecordingStreamBuilder::new("rerun_example_annotation_context_rects").memory()?;

    // Log an annotation context to assign a label and color to each class
    let annotation = AnnotationContext::new([
        AnnotationInfo {
            id: 1,
            label: Some(Label("red".into())),
            color: Some(Color::from(0xff000000)),
        },
        AnnotationInfo {
            id: 2,
            label: Some(Label("green".into())),
            color: Some(Color::from(0x00ff0000)),
        },
    ]);

    MsgSender::from_archetype("/", &annotation)?.send(&rec)?;

    // Log a batch of 2 rectangles with different class IDs
    MsgSender::new("detections")
        .with_component(&[
            Rect2D::XYWH(Vec4D([-2., -2., 3., 3.]).into()),
            Rect2D::XYWH(Vec4D([0., 0., 2., 2.]).into()),
        ])?
        .with_component(&[ClassId::from(1), ClassId::from(2)])?
        .send(&rec)?;

    // Log an extra rect to set the view bounds
    MsgSender::new("bounds")
        .with_component(&[Rect2D::XCYCWH(Vec4D([0.0, 0.0, 5.0, 5.0]).into())])?
        .send(&rec)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
