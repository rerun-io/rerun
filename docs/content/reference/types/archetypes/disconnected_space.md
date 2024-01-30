---
title: "DisconnectedSpace"
---

Spatially disconnect this entity from its parent.

Specifies that the entity path at which this is logged is spatially disconnected from its parent,
making it impossible to transform the entity path into its parent's space and vice versa.
It *only* applies to space views that work with spatial transformations, i.e. 2D & 3D space views.
This is useful for specifying that a subgraph is independent of the rest of the scene.

## Components

**Required**: [`DisconnectedSpace`](../components/disconnected_space.md)

## Links
 * 🌊 [C++ API docs for `DisconnectedSpace`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1archetypes_1_1DisconnectedSpace.html)
 * 🐍 [Python API docs for `DisconnectedSpace`](https://ref.rerun.io/docs/python/stable/common/archetypes#rerun.archetypes.DisconnectedSpace)
 * 🦀 [Rust API docs for `DisconnectedSpace`](https://docs.rs/rerun/latest/rerun/archetypes/struct.DisconnectedSpace.html)

## Example

### Disconnected Space

code-example: disconnected_space

<center>
<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/disconnected_space/b8f95b0e32359de625a765247c84935146c1fba9/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/disconnected_space/b8f95b0e32359de625a765247c84935146c1fba9/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/disconnected_space/b8f95b0e32359de625a765247c84935146c1fba9/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/disconnected_space/b8f95b0e32359de625a765247c84935146c1fba9/1200w.png">
  <img src="https://static.rerun.io/disconnected_space/b8f95b0e32359de625a765247c84935146c1fba9/full.png" width="640">
</picture>
</center>

