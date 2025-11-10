<!--[metadata]
title = "Raw mesh"
thumbnail = "https://static.rerun.io/raw-mesh/7731418dda47e15dbfc0f9a2c32673909071cb40/480w.png"
thumbnail_dimensions = [480, 480]
-->

This example demonstrates how to use the Rerun SDK to construct and log raw 3D meshes (so-called "triangle soups") programmatically from scratch.

This example shows how to create mesh geometry by manually defining vertices, normals, colors, and texture coordinates for various geometric primitives, each demonstrating different features of the [`Mesh3D`](https://rerun.io/docs/reference/types/archetypes/mesh3d) archetype.

If you want to log existing mesh files (like GLTF, OBJ, STL, etc.), use the [`Asset3D`](https://rerun.io/docs/reference/types/archetypes/asset3d) archetype instead.

<picture>
  <img src="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/1200w.png">
</picture>

## Geometric Primitives

The example generates several geometric primitives, each showcasing different `Mesh3D` features:

- **Cube**: Per-vertex colors (each face has a different color)
- **Pyramid**: UV texture coordinates with a procedural checkerboard texture
- **Sphere**: Vertex normals for smooth shading
- **Icosahedron**: Flat shading (no vertex normals)

## Running

```bash
cargo run --release -p raw_mesh
```

You can customize the sphere subdivisions for more or less detail:

```bash
cargo run --release -p raw_mesh -- --sphere-subdivisions 64
```
