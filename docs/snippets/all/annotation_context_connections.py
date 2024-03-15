"""Log annotation context with connections between keypoints."""

import rerun as rr
from rerun.datatypes import ClassDescription

rr.init("rerun_example_annotation_context_connections", spawn=True)

rr.log(
    "/",
    rr.AnnotationContext(
        [
            ClassDescription(
                info=0,
                keypoint_annotations=[
                    (0, "zero", (255, 0, 0)),
                    (1, "one", (0, 255, 0)),
                    (2, "two", (0, 0, 255)),
                    (3, "three", (255, 255, 0)),
                ],
                keypoint_connections=[(0, 2), (1, 2), (2, 3)],
            )
        ]
    ),
    static=True,
)

rr.log(
    "points",
    rr.Points3D(
        [
            (0, 0, 0),
            (50, 0, 20),
            (100, 100, 30),
            (0, 50, 40),
        ],
        class_ids=[0],
        keypoint_ids=[0, 1, 2, 3],
    ),
)
