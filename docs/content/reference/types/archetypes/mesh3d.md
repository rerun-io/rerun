---
title: "Mesh3D"
---

A 3D triangle mesh as specified by its per-mesh and per-vertex properties.

## Components

**Required**: [`Position3D`](../components/position3d.md)

**Recommended**: [`MeshProperties`](../components/mesh_properties.md), [`Vector3D`](../components/vector3d.md)

**Optional**: [`Color`](../components/color.md), [`Texcoord2D`](../components/texcoord2d.md), [`Material`](../components/material.md), [`TensorData`](../components/tensor_data.md), [`ClassId`](../components/class_id.md)

## Links
 * 🌊 [C++ API docs for `Mesh3D`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1archetypes_1_1Mesh3D.html)
 * 🐍 [Python API docs for `Mesh3D`](https://ref.rerun.io/docs/python/stable/common/archetypes#rerun.archetypes.Mesh3D)
 * 🦀 [Rust API docs for `Mesh3D`](https://docs.rs/rerun/latest/rerun/archetypes/struct.Mesh3D.html)

## Examples

### Simple indexed 3D mesh

snippet: mesh3d_indexed

<center>
<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/mesh3d_simple/e1e5fd97265daf0d0bc7b782d862f19086fd6975/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/mesh3d_simple/e1e5fd97265daf0d0bc7b782d862f19086fd6975/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/mesh3d_simple/e1e5fd97265daf0d0bc7b782d862f19086fd6975/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/mesh3d_simple/e1e5fd97265daf0d0bc7b782d862f19086fd6975/1200w.png">
  <img src="https://static.rerun.io/mesh3d_simple/e1e5fd97265daf0d0bc7b782d862f19086fd6975/full.png" width="640">
</picture>
</center>

### 3D mesh with partial updates

snippet: mesh3d_partial_updates

<center>
<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/mesh3d_partial_updates/a11e4accb0257dcd9531867b7e1d6fd5e3bee5c3/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/mesh3d_partial_updates/a11e4accb0257dcd9531867b7e1d6fd5e3bee5c3/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/mesh3d_partial_updates/a11e4accb0257dcd9531867b7e1d6fd5e3bee5c3/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/mesh3d_partial_updates/a11e4accb0257dcd9531867b7e1d6fd5e3bee5c3/1200w.png">
  <img src="https://static.rerun.io/mesh3d_partial_updates/a11e4accb0257dcd9531867b7e1d6fd5e3bee5c3/full.png" width="640">
</picture>
</center>

