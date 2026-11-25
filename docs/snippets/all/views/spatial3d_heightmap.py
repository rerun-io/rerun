"""Draw a heightmap as a triangle mesh with one vertex per grid cell."""

import numpy as np

import rerun as rr

rr.init("rerun_example_spatial3d_heightmap", spawn=True)

rows, cols = 100, 120
cell_size = 0.1
y, x = np.mgrid[0:rows, 0:cols] * cell_size
height = 0.5 * np.sin(x) * np.cos(y)

positions = np.column_stack([x.ravel(), y.ravel(), height.ravel()])

index = np.arange(rows * cols).reshape(rows, cols)
top_left = index[:-1, :-1].ravel()
top_right = index[:-1, 1:].ravel()
bottom_left = index[1:, :-1].ravel()
bottom_right = index[1:, 1:].ravel()
triangles = np.concatenate([
    np.column_stack([top_left, bottom_left, top_right]),
    np.column_stack([top_right, bottom_left, bottom_right]),
])

rr.log(
    "terrain",
    rr.Mesh3D(vertex_positions=positions, triangle_indices=triangles),
)
