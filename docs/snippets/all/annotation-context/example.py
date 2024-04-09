import rerun as rr

# Annotation context with two classes, using two labeled classes, of which ones defines a color.
rr.log(
    "masks",  # Applies to all entities below "masks".
    rr.AnnotationContext(
        [
            rr.AnnotationInfo(id=0, label="Background"),
            rr.AnnotationInfo(id=1, label="Person", color=(255, 0, 0)),
        ],
    ),
    static=True,
)

# Annotation context with simple keypoints & keypoint connections.
rr.log(
    "detections",  # Applies to all entities below "detections".
    rr.ClassDescription(
        info=rr.AnnotationInfo(0, label="Snake"),
        keypoint_annotations=[rr.AnnotationInfo(id=i, color=(0, 255 / 9 * i, 0)) for i in range(10)],
        keypoint_connections=[(i, i + 1) for i in range(9)],
    ),
    static=True,
)
