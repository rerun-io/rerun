"""Log a simple 3D mesh with several leaf-transforms which instantiate the mesh several times and will not affect its children."""

import rerun as rr

rr.init("rerun_example_mesh3d_leaf_transforms3d", spawn=True)
rr.set_time_sequence("frame", 0)

rr.log(
    "shape",
    rr.Mesh3D(
        vertex_positions=[[1, 1, 1], [-1, -1, 1], [-1, 1, -1], [1, -1, -1]],
        triangle_indices=[[0, 1, 2], [0, 1, 3], [0, 2, 3], [1, 2, 3]],
        vertex_colors=[[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0]],
    ),
)
# This box will not be affected by its parent's leaf transforms!
rr.log(
    "shape/box",
    rr.Boxes3D(half_sizes=[[5.0, 5.0, 5.0]]),
)

for i in range(0, 100):
    rr.set_time_sequence("frame", i)
    rr.log(
        "shape",
        rr.LeafTransforms3D(
            translations=[[2, 0, 0], [0, 2, 0], [0, -2, 0], [-2, 0, 0]],
            rotation_axis_angles=rr.RotationAxisAngle([0, 0, 1], rr.Angle(deg=i * 2)),
        ),
    )
