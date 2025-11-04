"""Log a simple colored triangle, then update its vertices' positions each frame."""

import numpy as np
import rerun as rr

rr.init("rerun_example_mesh3d_partial_updates", spawn=True)

vertex_positions = np.array([[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], dtype=np.float32)

# Log the initial state of our triangle
rr.set_time("frame", sequence=0)
rr.log(
    "triangle",
    rr.Mesh3D(
        vertex_positions=vertex_positions,
        vertex_normals=[0.0, 0.0, 1.0],
        vertex_colors=[[255, 0, 0], [0, 255, 0], [0, 0, 255]],
    ),
)

# Only update its vertices' positions each frame:
for i in range(1, 300):
    factor = np.abs(np.sin(i * 0.04))
    rr.set_time("frame", sequence=i)
    rr.log("triangle", rr.Mesh3D.from_fields(vertex_positions=vertex_positions * factor))
