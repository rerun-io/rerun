"""Log a simple colored triangle."""
import rerun as rr

rr.init("rerun-example-mesh", spawn=True)

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
)
