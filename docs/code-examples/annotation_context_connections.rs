//! Log some very simple points.
use rerun::components::{
    AnnotationContext, AnnotationInfo, ClassDescription, ClassId, Color, KeypointId, Label, Point3D,
};
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) =
        RecordingStreamBuilder::new("annotation_context_connections").memory()?;

    // Log an annotation context to assign a label and color to each class
    // Create a class description with labels and color for each keypoint ID as well as some
    // connections between keypoints.
    let mut class_desc = ClassDescription::default();
    class_desc.keypoint_map.insert(
        KeypointId(0),
        AnnotationInfo {
            id: 0,
            label: Some(Label("zero".into()).into()),
            color: Some(Color::from_rgb(255, 0, 0).into()),
        },
    );
    class_desc.keypoint_map.insert(
        KeypointId(1),
        AnnotationInfo {
            id: 1,
            label: Some(Label("one".into()).into()),
            color: Some(Color::from_rgb(0, 255, 0).into()),
        },
    );
    class_desc.keypoint_map.insert(
        KeypointId(2),
        AnnotationInfo {
            id: 2,
            label: Some(Label("two".into()).into()),
            color: Some(Color::from_rgb(0, 0, 255).into()),
        },
    );
    class_desc.keypoint_map.insert(
        KeypointId(3),
        AnnotationInfo {
            id: 3,
            label: Some(Label("three".into()).into()),
            color: Some(Color::from_rgb(255, 255, 0).into()),
        },
    );
    class_desc.keypoint_connections = [(0, 2), (1, 2), (2, 3)]
        .into_iter()
        .map(|(a, b)| (KeypointId(a), KeypointId(b)))
        .collect();

    let mut annotation = AnnotationContext::default();
    annotation.class_map.insert(ClassId(0), class_desc);

    MsgSender::new("/")
        .with_component(&[annotation])?
        .send(&rec_stream)?;

    // Log some points with different keypoint IDs
    let points = [
        [0., 0., 0.],
        [50., 0., 20.],
        [100., 100., 30.],
        [0., 50., 40.],
    ]
    .into_iter()
    .map(Point3D::from)
    .collect::<Vec<_>>();

    MsgSender::new("points")
        .with_component(&points)?
        .with_component(&[KeypointId(0), KeypointId(1), KeypointId(2), KeypointId(3)])?
        .with_splat(ClassId(0))?
        .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;

    Ok(())
}
