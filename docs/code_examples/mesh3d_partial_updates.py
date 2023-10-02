"""Log a simple colored triangle, then update its vertices' positions each frame."""
import numpy as np
import rerun as rr
from rerun.components import Position3DBatch

rr.init("rerun_example_mesh3d_partial_updates", spawn=True)

vertex_positions = np.array([[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], dtype=np.float32)

# Log the initial state of our triangle
rr.set_time_sequence("frame", 0)
rr.log(
    "triangle",
    rr.Mesh3D(
        vertex_positions=vertex_positions,
        vertex_normals=[0.0, 0.0, 1.0],
        vertex_colors=[[255, 0, 0], [0, 255, 0], [0, 0, 255]],
    ),
)

# Only update its vertices' positions each frame
factors = np.abs(np.sin(np.arange(1, 300, dtype=np.float32) * 0.04))
for i, factor in enumerate(factors):
    rr.set_time_sequence("frame", i)
    rr.log_components("triangle", [Position3DBatch(vertex_positions * factor)])
