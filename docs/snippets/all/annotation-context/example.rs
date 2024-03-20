// Annotation context with two classes, using two labeled classes, of which ones defines a color.
MsgSender::new("masks") // Applies to all entities below "masks".
    .with_timeless(true)
    .with_component(&[AnnotationContext {
        class_map: [
            ClassDescription {
                info: AnnotationInfo {
                    id: 0,
                    label: Some(Label("Background".into())),
                    color: None,
                },
                ..Default::default()
            },
            ClassDescription {
                info: AnnotationInfo {
                    id: 0,
                    label: Some(Label("Person".into())),
                    color: Some(Color(0xFF000000)),
                },
                ..Default::default()
            },
        ]
        .into_iter()
        .map(|class| (ClassId(class.info.id), class))
        .collect(),
    }])?
    .send(rec)?;

// Annotation context with simple keypoints & keypoint connections.
MsgSender::new("detections") // Applies to all entities below "detections".
    .with_timeless(true)
    .with_component(&[AnnotationContext {
        class_map: std::iter::once((
            ClassId(0),
            ClassDescription {
                info: AnnotationInfo {
                    id: 0,
                    label: Some(Label("Snake".into())),
                    color: None,
                },
                keypoint_map: (0..10)
                    .map(|i| AnnotationInfo {
                        id: i,
                        label: None,
                        color: Some(Color::from_rgb(0, (255 / 9 * i) as u8, 0)),
                    })
                    .map(|keypoint| (KeypointId(keypoint.id), keypoint))
                    .collect(),
                keypoint_connections: (0..9)
                    .map(|i| (KeypointId(i), KeypointId(i + 1)))
                    .collect(),
            },
        ))
        .collect(),
    }])?
    .send(rec)?;
