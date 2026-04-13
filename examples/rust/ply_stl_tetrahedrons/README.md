<!--[metadata]
title = "PLY And STL Tetrahedrons"
tags = ["PLY", "STL", "Mesh3D", "Points3D", "Points2D", "Asset3D"]
-->

This example loads a handful of tiny ASCII geometry files and logs them with the Rust SDK.

It demonstrates:

- a `.ply` point cloud read as [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d)
- a `.ply` mesh read as [`Mesh3D`](https://www.rerun.io/docs/reference/types/archetypes/mesh3d)
- an `x/y`-only `.ply` mesh read as `Mesh3D` and flattened onto `z=0`
- an `x/y`-only `.ply` point cloud read as [`Points2D`](https://www.rerun.io/docs/reference/types/archetypes/points2d)
- an ASCII `.stl` tetrahedron read as [`Asset3D`](https://www.rerun.io/docs/reference/types/archetypes/asset3d)

All files are generated as tetrahedrons or tetrahedron projections and live in this example's `data/` directory.

## Run the code

```bash
cargo run -p ply_stl_tetrahedrons
```
