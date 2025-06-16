<!--[metadata]
title = "Raw mesh"
thumbnail = "https://static.rerun.io/raw-mesh/7731418dda47e15dbfc0f9a2c32673909071cb40/480w.png"
thumbnail_dimensions = [480, 480]
-->

This example demonstrates how to use the Rerun SDK to log raw 3D meshes (so-called "triangle soups") and their transform hierarchy. Simple material properties are supported.

Note that while this example loads GLTF meshes to illustrate [`Mesh3D`](https://rerun.io/docs/reference/types/archetypes/mesh3d)'s abilitites, you can also send various kinds of mesh assets
directly via [`Asset3D`](https://rerun.io/docs/reference/types/archetypes/asset3d).

<!-- TODO(#1957): How about we load something elseto avoid confusion? -->

<picture>
  <img src="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/1200w.png">
</picture>

```bash
cargo run --release
```
