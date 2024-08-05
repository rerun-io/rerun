"""Log a batch of ellipsoids."""

import rerun as rr

rr.init("rerun_example_ellipsoid_batch", spawn=True)

# Let's build a snowman!
belly_z = 2.5
head_z = 4.5
rr.log(
    "batch",
    rr.Ellipsoids3D(
        centers=[
            [0.0, 0.0, 0.0],
            [0.0, 0.0, belly_z],
            [0.0, 0.0, head_z],
            [-0.6, -0.77, head_z],
            [0.6, -0.77, head_z],
        ],
        half_sizes=[
            [2.0, 2.0, 2.0],
            [1.5, 1.5, 1.5],
            [1.0, 1.0, 1.0],
            [0.15, 0.15, 0.15],
            [0.15, 0.15, 0.15],
        ],
        colors=[
            (255, 255, 255),
            (255, 255, 255),
            (255, 255, 255),
            (0, 0, 0),
            (0, 0, 0),
        ],
        fill_mode="solid",
    ),
)
