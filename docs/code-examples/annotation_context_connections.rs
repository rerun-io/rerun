//! Log some very simple points.

use rerun::archetypes::{AnnotationContext, Points3D};
use rerun::datatypes::{ClassDescription, Color, KeypointPair};
use rerun::RecordingStreamBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) =
        RecordingStreamBuilder::new("rerun_example_annotation_context_connections").memory()?;

    // Log an annotation context to assign a label and color to each class
    // Create a class description with labels and color for each keypoint ID as well as some
    // connections between keypoints.
    rec.log(
        "/",
        &AnnotationContext::new([ClassDescription {
            info: 0.into(),
            keypoint_annotations: vec![
                (0, "zero", Color::from(0xFF0000FF)).into(),
                (1, "one", Color::from(0x00FF00FF)).into(),
                (2, "two", Color::from(0x0000FFFF)).into(),
                (3, "three", Color::from(0xFFFF00FF)).into(),
            ],
            keypoint_connections: KeypointPair::vec_from([(0, 2), (1, 2), (2, 3)]),
        }]),
    )?;

    // Log some points with different keypoint IDs
    rec.log(
        "points",
        &Points3D::new([
            [0., 0., 0.],
            [50., 0., 20.],
            [100., 100., 30.],
            [0., 50., 40.],
        ])
        .with_keypoint_ids([0, 1, 2, 3])
        .with_class_ids([0]),
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
