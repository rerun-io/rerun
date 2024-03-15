//! Log annotation context with connections between keypoints.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_annotation_context_connections")
        .spawn()?;

    // Log an annotation context to assign a label and color to each class
    // Create a class description with labels and color for each keypoint ID as well as some
    // connections between keypoints.
    rec.log_static(
        "/",
        &rerun::AnnotationContext::new([rerun::ClassDescription {
            info: 0.into(),
            keypoint_annotations: vec![
                (0, "zero", rerun::Rgba32::from_rgb(255, 0, 0)).into(),
                (1, "one", rerun::Rgba32::from_rgb(0, 255, 0)).into(),
                (2, "two", rerun::Rgba32::from_rgb(0, 0, 255)).into(),
                (3, "three", rerun::Rgba32::from_rgb(255, 255, 0)).into(),
            ],
            keypoint_connections: rerun::KeypointPair::vec_from([(0, 2), (1, 2), (2, 3)]),
        }]),
    )?;

    // Log some points with different keypoint IDs
    rec.log(
        "points",
        &rerun::Points3D::new([
            [0., 0., 0.],
            [50., 0., 20.],
            [100., 100., 30.],
            [0., 50., 40.],
        ])
        .with_keypoint_ids([0, 1, 2, 3])
        .with_class_ids([0]),
    )?;

    Ok(())
}
