//! Log rectangles with different colors and labels.
use rerun::{
    components::{
        AnnotationContext, AnnotationInfo, ClassDescription, ClassId, Color, Label, Rect2D,
    },
    datatypes::Vec4D,
    MsgSender, RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new("annotation_context_rects").memory()?;

    // Log an annotation context to assign a label and color to each class
    let mut annotation = AnnotationContext::default();
    annotation.class_map.insert(
        ClassId(1),
        ClassDescription {
            info: AnnotationInfo {
                id: 1,
                label: Some(Label("red".into()).into()),
                color: Some(Color::from_rgb(255, 0, 0).into()),
            },
            ..Default::default()
        },
    );
    annotation.class_map.insert(
        ClassId(2),
        ClassDescription {
            info: AnnotationInfo {
                id: 2,
                label: Some(Label("green".into()).into()),
                color: Some(Color::from_rgb(0, 255, 0).into()),
            },
            ..Default::default()
        },
    );

    MsgSender::new("/")
        .with_component(&[annotation])?
        .send(&rec_stream)?;

    // Log a batch of 2 rectangles with different class IDs
    MsgSender::new("detections")
        .with_component(&[
            Rect2D::XYWH(Vec4D([-2., -2., 3., 3.]).into()),
            Rect2D::XYWH(Vec4D([0., 0., 2., 2.]).into()),
        ])?
        .with_component(&[ClassId(1), ClassId(2)])?
        .send(&rec_stream)?;

    // Log an extra rect to set the view bounds
    MsgSender::new("bounds")
        .with_component(&[Rect2D::XCYCWH(Vec4D([0.0, 0.0, 5.0, 5.0]).into())])?
        .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
