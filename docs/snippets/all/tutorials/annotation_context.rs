use rerun::{
    AnnotationContext, AnnotationInfo, ClassDescription, Rgba32,
    datatypes::{ClassDescriptionMapElem, KeypointId},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_annotation_context_connections")
        .spawn()?;

    // Annotation context with two classes, using two labeled classes, of which ones defines a
    // color.
    rec.log_static(
        "masks", // Applies to all entities below "masks".
        &AnnotationContext::new([
            ClassDescriptionMapElem::from((0, "Background")),
            ClassDescriptionMapElem::from((1, "Person", Rgba32::from_rgb(255, 0, 0))),
        ]),
    )?;

    // Annotation context with simple keypoints & keypoint connections.
    rec.log_static(
        "detections", // Applies to all entities below "detections".
        &AnnotationContext::new([ClassDescription {
            info: (0, "Snake").into(),
            keypoint_annotations: (0..10)
                .map(|i| AnnotationInfo {
                    id: i,
                    label: None,
                    color: Some(Rgba32::from_rgb(0, (28 * i) as u8, 0)),
                })
                .collect(),
            keypoint_connections: (0..9)
                .map(|i| (KeypointId(i), KeypointId(i + 1)))
                .map(Into::into)
                .collect(),
        }]),
    )?;

    Ok(())
}
