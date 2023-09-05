"""Log a simple colored triangle."""
import numpy as np
import rerun as rr

rr.init("rerun_example_mesh", spawn=True)

rr.log_mesh(
    "triangle",
    positions=[
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
    ],
    indices=[0, 1, 2],
    normals=[
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
    ],
    vertex_colors=[
        [255, 0, 0],
        [0, 255, 0],
        [0, 0, 255],
    ],
    mesh_id=np.repeat(0, 16).astype(np.uint8),
)
