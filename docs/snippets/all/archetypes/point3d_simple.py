"""Log some very simple points."""

import rerun as rr

rr.init("rerun_example_points3d", spawn=True)

rr.log(
    "points",
    rr.Points3D(
        [[0, 0, 0], [1, 1, 1]],
        radii=10,
        colors=[1, 1, 1],
        labels="kek",
        class_ids=30,
        keypoint_ids=42,
    ),
)
