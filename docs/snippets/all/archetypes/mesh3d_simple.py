"""Log a simple colored triangle."""

import rerun as rr

rr.init("rerun_example_mesh3d", spawn=True)

rr.log(
    "triangle",
    rr.Mesh3D(
        vertex_positions=[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        vertex_normals=[0.0, 0.0, 1.0],
        vertex_colors=[[255, 0, 0], [0, 255, 0], [0, 0, 255]],
    ),
)
