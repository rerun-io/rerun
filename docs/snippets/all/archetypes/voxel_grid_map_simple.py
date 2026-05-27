"""Log a simple sparse voxel grid map."""

import numpy as np

import rerun as rr

voxel_indices = np.array(
    [
        [-1, 0, 0],
        [1, 0, 0],
        [1, 1, 0],
        [3, 0, 0],
        [3, 0, 1],
        [4, 0, 1],
    ],
    dtype=np.int32,
)
values = np.array([0.0, 0.2, 0.4, 0.6, 0.8, 1.0], dtype=np.float32)

rr.init("rerun_example_voxel_grid_map_simple", spawn=True)

rr.log(
    "world/voxels",
    rr.VoxelGridMap(
        voxel_indices,
        0.25,
        values=values,
        value_range=[0.0, 1.0],
        colormap=rr.components.Colormap.Turbo,
        translation=[-0.5, -0.5, 0.0],
    ),
)
