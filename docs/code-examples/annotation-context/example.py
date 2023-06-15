# Annotation context with two classes, using two labled classes, of which ones defines a color.
# Applies to all entities below "masks".
rr.log_annotation_context(
    "masks",
    [
        rr.AnnotationInfo(id=0, label="Background"),
        rr.AnnotationInfo(id=1, label="Person", color=(0, 0, 0)),
    ],
)

# Annotation context with simple keypoints & keypoint connections.
# Applies to all entities below "detections".
rr.log_annotation_context(
    "detections",
    rr.ClassDescription(
        info=rr.AnnotationInfo(label="Snake"),
        keypoint_annotations=[rr.AnnotationInfo(id=i, color=(255 / 10 * i, 0, 0)) for i in range(10)],
        keypoint_connections=[(i, i + 1) for i in range(9)],
    ),
)
