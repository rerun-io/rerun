---
title: Visualize depth images, heightmaps, and voxels
order: 280
description: Turn a depth image into a 3D point cloud, show heightmaps and elevation grids as meshes, and show voxel grids and dense volumes
---

## Depth image to 3D point cloud

Log a [`DepthImage`](../../reference/types/archetypes/depth_image.md) under an entity with a [`Pinhole`](../../reference/types/archetypes/pinhole.md) camera.
The 3D view then projects every pixel into 3D, so you get a point cloud without computing it yourself.
Set `meter` to the number of depth units per meter (for example `1000` for depth in millimeters).

snippet: archetypes/depth_image_3d

To place the camera in the world, log a [`Transform3D`](../../reference/types/archetypes/transform3d.md) to the camera entity.
The same depth image is also shown as an image in a 2D view.
`point_fill_ratio` sets the size of the points, and `colormap` and `depth_range` set their colors.

## Heightmaps and elevation grids

There is no built-in heightmap or surface archetype.
Build a [`Mesh3D`](../../reference/types/archetypes/mesh3d.md) from the grid: one vertex per cell, and two triangles per grid square.

```python
import numpy as np
import rerun as rr

rr.init("rerun_example_heightmap", spawn=True)

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

rr.log("terrain", rr.Mesh3D(vertex_positions=positions, triangle_indices=triangles))
```

The mesh has no normals, so it is drawn without shading.
Pass `vertex_normals` to get lighting, and `vertex_colors` to color it by height.
See [Control mesh appearance](mesh-appearance.md).

For a quick look, you can instead log the grid as a [`Points3D`](../../reference/types/archetypes/points3d.md) with the same `positions`, or as a 2D [`Image`](../../reference/types/archetypes/image.md) or [`DepthImage`](../../reference/types/archetypes/depth_image.md) with a colormap.

## Voxels

For sparse voxel grids, such as occupancy maps, use [`VoxelGridMap`](../../reference/types/archetypes/voxel_grid_map.md).
It takes integer voxel indices, a voxel size, and optional values with a colormap.
`VoxelGridMap` is unstable: its data format may change in future releases.

snippet: archetypes/voxel_grid_map_simple

You can also draw voxels as [`Boxes3D`](../../reference/types/archetypes/boxes3d.md) with `fill_mode="solid"`, one box per voxel.
This works for a moderate number of voxels.

## Dense volumes

For dense 3D scalar fields, such as CT or MRI scans, signed distance fields, or occupancy probabilities, use [`Volume3D`](../../reference/types/archetypes/volume3d.md).
It is rendered with ray marching.
`Volume3D` is unstable: its data format may change in future releases.

snippet: archetypes/volume3d_simple

For 2D occupancy grids, for example from a robot's map, see [`GridMap`](../../reference/types/archetypes/grid_map.md).
