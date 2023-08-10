//! Log some very simple points.

use rerun::archetypes::{AnnotationContext, Points3D};
use rerun::datatypes::{AnnotationInfo, ClassDescription, Color, KeypointPair, Label};
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new(env!("CARGO_BIN_NAME")).memory()?;

    // Log an annotation context to assign a label and color to each class
    // Create a class description with labels and color for each keypoint ID as well as some
    // connections between keypoints.
    let annotation = AnnotationContext::new([
        ClassDescription {
            info: AnnotationInfo {
                id: 0,
                label: Some(Label("zero".into())),
                color: Some(Color::from_rgb(255, 0, 0)),
            },
            keypoint_connections: KeypointPair::vec_from([(0, 2), (1, 2), (2, 3)]),
            ..Default::default()
        },
        AnnotationInfo {
            id: 1,
            label: Some(Label("one".into())),
            color: Some(Color::from_rgb(0, 255, 0)),
        }
        .into(),
        AnnotationInfo {
            id: 2,
            label: Some(Label("two".into())),
            color: Some(Color::from_rgb(0, 0, 255)),
        }
        .into(),
        AnnotationInfo {
            id: 3,
            label: Some(Label("three".into())),
            color: Some(Color::from_rgb(255, 255, 0)),
        }
        .into(),
    ]);

    MsgSender::from_archetype("/", &annotation)?.send(&rec_stream)?;

    // Log some points with different keypoint IDs
    MsgSender::from_archetype(
        "points",
        &Points3D::new([
            [0., 0., 0.],
            [50., 0., 20.],
            [100., 100., 30.],
            [0., 50., 40.],
        ])
        .with_keypoint_ids([0, 1, 2, 3])
        .with_class_ids([0]),
    )?
    .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;

    Ok(())
}
