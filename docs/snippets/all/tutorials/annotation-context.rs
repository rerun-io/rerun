// Annotation context with two classes, using two labeled classes, of which ones defines a
// color.
rr.log_static(
    "masks", // Applies to all entities below "masks".
    &AnnotationContext(
        [
            ClassDescription {
                info: AnnotationInfo {
                    id: 0,
                    label: Some(Utf8("Background".into())),
                    color: None,
                },
                ..Default::default()
            },
            ClassDescription {
                info: AnnotationInfo {
                    id: 0,
                    label: Some(Utf8("Person".into())),
                    color: Some(Rgba32(0xFF000000)),
                },
                ..Default::default()
            },
        ]
        .into_iter()
        .map(|class| ClassDescriptionMapElem {
            class_id: ClassId(class.info.id),
            class_description: class,
        })
        .collect(),
    ),
)?;

// Annotation context with simple keypoints & keypoint connections.
rr.log_static(
    "detections", // Applies to all entities below "detections".
    &AnnotationContext(
        [ClassDescription {
            info: AnnotationInfo {
                id: 0,
                label: Some(Utf8("Snake".into())),
                color: None,
            },
            keypoint_annotations: (0..10)
                .map(|i| AnnotationInfo {
                    id: i,
                    label: None,
                    color: Some(Rgba32::from_rgb(0, (255 / 9 * i) as u8, 0)),
                })
                .collect(),
            keypoint_connections: (0..9)
                .map(|i| (KeypointId(i), KeypointId(i + 1)))
                .map(Into::into)
                .collect(),
        }]
        .into_iter()
        .map(|class| ClassDescriptionMapElem {
            class_id: ClassId(class.info.id),
            class_description: class,
        })
        .collect(),
    ),
)?;
