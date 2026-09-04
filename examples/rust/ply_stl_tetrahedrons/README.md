<!--[metadata]
title = "Asset3D PLY mesh rendering"
tags = ["2D", "3D", "Mesh", "Point cloud"]
-->

This example logs raw `.ply` mesh files as [`Asset3D`](https://www.rerun.io/docs/reference/types/archetypes/asset3d) with the Rust SDK and relies on the Viewer to render them as meshes.

It also includes point-cloud PLY files and an STL mesh for comparison.

It demonstrates:

- a `.ply` point cloud read as [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d)
- a `.ply` mesh read as [`Asset3D`](https://www.rerun.io/docs/reference/types/archetypes/asset3d)
- an `x/y`-only `.ply` mesh read as `Asset3D` and flattened onto `z=0` by the viewer
- an `x/y`-only `.ply` point cloud read as [`Points2D`](https://www.rerun.io/docs/reference/types/archetypes/points2d)
- an ASCII `.stl` tetrahedron read as [`Asset3D`](https://www.rerun.io/docs/reference/types/archetypes/asset3d)

All files are generated as tetrahedrons or tetrahedron projections and live in this example's `data/` directory.

## Run the code

```bash
cargo run -p ply_stl_tetrahedrons
```

This spawns a Viewer and logs into it.
Pass `--help` for the other ways to consume the data, such as `--save` or `--serve`.
