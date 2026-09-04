"""Log an image as a 3D quad."""

import numpy as np

import rerun as rr

rr.init("rerun_example_mesh3d_image", spawn=True)

# Simple gradient image
image = np.array(
    [[[x, y, 0] for x in range(256)] for y in range(256)],
    dtype=np.uint8,
)

top_left = [1.0, 1.0, 1.0]
top_right = [1.0, 0.0, 1.0]
bottom_right = [1.0, 0.0, 0.0]
bottom_left = [1.0, 1.0, 0.0]
alpha = 255

# Inset by half a pixel so opposite edges of the image don't leak
# onto the border.
height, width = image.shape[:2]
u0, v0 = 0.5 / width, 0.5 / height
u1, v1 = 1.0 - u0, 1.0 - v0
rr.log(
    "image",
    rr.Mesh3D(
        vertex_positions=[top_left, top_right, bottom_right, bottom_left],
        vertex_texcoords=[[u0, v0], [u1, v0], [u1, v1], [u0, v1]],
        triangle_indices=[[0, 2, 1], [0, 3, 2]],
        albedo_texture=image,
        albedo_factor=[255, 255, 255, alpha],
    ),
)

# Example 3D data
rng = np.random.default_rng(1)
strips = []
colors = []
for _ in range(20):
    u, v = rng.uniform(0.1, 0.9, size=2)
    color = rng.integers(40, 255, size=3)
    strip = []
    for step in range(40):
        t = step / (40 - 1)
        u = float(np.clip(u + rng.normal(0, 0.01), 0.0, 1.0))
        v = float(np.clip(v + rng.normal(0, 0.01), 0.0, 1.0))
        strip.append([1.0 - t, 1.0 - u, 1.0 - v])

    strips.append(strip)
    colors.append(color)

rr.log("tracks", rr.LineStrips3D(strips, colors=colors, radii=0.002))

rr.log(
    "time-axis",
    rr.Arrows3D(
        origins=[[0.0, 1.0, 0.0]],
        vectors=[[1.0, 0.0, 0.0]],
        labels=["Time"],
        colors=[[255, 255, 255]],
        radii=[0.005],
    ),
)
