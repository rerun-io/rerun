import rerun as rr

rr.init("rerun-example-annotation_context_connections", spawn=True)

rr.log_annotation_context(
    "/",
    rr.ClassDescription(
        keypoint_annotations=[
            rr.AnnotationInfo(0, "zero", (255, 0, 0)),
            rr.AnnotationInfo(1, "one", (0, 255, 0)),
            rr.AnnotationInfo(2, "two", (0, 0, 255)),
            rr.AnnotationInfo(3, "three", (255, 255, 0)),
        ],
        keypoint_connections=[(0, 2), (1, 2), (2, 3)],
    ),
)

rr.log_points(
    "points",
    [
        (0, 0, 0),
        (50, 0, 20),
        (100, 100, 30),
        (0, 50, 40),
    ],
    keypoint_ids=[0, 1, 2, 3],
)
