"""Log a simple 3D mesh with several instance pose transforms which instantiate the mesh several times and will not affect its children (known as mesh instancing)."""

import rerun as rr

rr.init("rerun_example_mesh3d_instancing", spawn=True)
rr.set_time("frame", sequence=0)

rr.log(
    "shape",
    rr.Mesh3D(
        vertex_positions=[[1, 1, 1], [-1, -1, 1], [-1, 1, -1], [1, -1, -1]],
        triangle_indices=[[0, 2, 1], [0, 3, 1], [0, 3, 2], [1, 3, 2]],
        vertex_colors=[[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0]],
    ),
)
# This box will not be affected by its parent's instance poses!
rr.log(
    "shape/box",
    rr.Boxes3D(half_sizes=[[5.0, 5.0, 5.0]]),
)

for i in range(100):
    rr.set_time("frame", sequence=i)
    rr.log(
        "shape",
        rr.InstancePoses3D(
            translations=[[2, 0, 0], [0, 2, 0], [0, -2, 0], [-2, 0, 0]],
            rotation_axis_angles=rr.RotationAxisAngle([0, 0, 1], rr.Angle(deg=i * 2)),
        ),
    )
