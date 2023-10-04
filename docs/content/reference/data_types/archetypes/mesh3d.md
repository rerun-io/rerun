---
title: "Mesh3D"
---

A 3D triangle mesh as specified by its per-mesh and per-vertex properties.

## Components

**Required**: [`Position3D`](../components/position3d.md)

**Recommended**: [`MeshProperties`](../components/mesh_properties.md), [`Vector3D`](../components/vector3d.md)

**Optional**: [`Color`](../components/color.md), [`Material`](../components/material.md), [`ClassId`](../components/class_id.md), [`InstanceKey`](../components/instance_key.md)

## Links
 * 🐍 [Python API docs for `Mesh3D`](https://ref.rerun.io/docs/python/HEAD/package/rerun/archetypes/mesh3d/)
 * 🦀 [Rust API docs for `Mesh3D`](https://docs.rs/rerun/0.9.0-alpha.6/rerun/archetypes/struct.Mesh3D.html)

## Examples

### Simple indexed 3D mesh

code-example: mesh3d_indexed

### 3D mesh with partial updates

code-example: mesh3d_partial_updates

