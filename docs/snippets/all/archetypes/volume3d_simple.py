"""Log a small volume: a soft sphere sampled on a 32³ grid."""

import numpy as np

import rerun as rr

SIZE = 32
RADIUS = np.float32(14.0)

rr.init("rerun_example_volume3d_simple", spawn=True)

# Squared distance from the center of the grid, per voxel, in voxel units.
axis = np.arange(SIZE, dtype=np.float32) - np.float32(0.5 * (SIZE - 1))
z, y, x = np.meshgrid(axis, axis, axis, indexing="ij")
distance_sq = x * x + y * y + z * z

# A soft sphere: 1 at the center, falling off to 0 at `RADIUS`.
values = np.maximum(
    np.float32(0.0), np.float32(1.0) - distance_sq / (RADIUS * RADIUS)
)

# Dimensions are ordered `[z, y, x]`. Only `f16` values are supported for now.
rr.log(
    "volume",
    rr.Volume3D(values.astype(np.float16), voxel_size=[0.1, 0.1, 0.1]),
)
