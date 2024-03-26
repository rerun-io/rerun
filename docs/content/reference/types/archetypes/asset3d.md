---
title: "Asset3D"
---

A prepacked 3D asset (`.gltf`, `.glb`, `.obj`, `.stl`, etc.).

## Components

**Required**: [`Blob`](../components/blob.md)

**Recommended**: [`MediaType`](../components/media_type.md)

**Optional**: [`OutOfTreeTransform3D`](../components/out_of_tree_transform3d.md)

## Links
 * 🌊 [C++ API docs for `Asset3D`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1archetypes_1_1Asset3D.html)
 * 🐍 [Python API docs for `Asset3D`](https://ref.rerun.io/docs/python/stable/common/archetypes#rerun.archetypes.Asset3D)
 * 🦀 [Rust API docs for `Asset3D`](https://docs.rs/rerun/latest/rerun/archetypes/struct.Asset3D.html)

## Examples

### Simple 3D asset

snippet: asset3d_simple

<picture data-inline-viewer="snippets/asset3d_simple">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/asset3d_simple/af238578188d3fd0de3e330212120e2842a8ddb2/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/asset3d_simple/af238578188d3fd0de3e330212120e2842a8ddb2/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/asset3d_simple/af238578188d3fd0de3e330212120e2842a8ddb2/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/asset3d_simple/af238578188d3fd0de3e330212120e2842a8ddb2/1200w.png">
  <img src="https://static.rerun.io/asset3d_simple/af238578188d3fd0de3e330212120e2842a8ddb2/full.png">
</picture>

### 3D asset with out-of-tree transform

snippet: asset3d_out_of_tree

