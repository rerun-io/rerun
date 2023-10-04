---
title: "DisconnectedSpace"
---

Specifies that the entity path at which this is logged is disconnected from its parent.

This is useful for specifying that a subgraph is independent of the rest of the scene.

If a transform or pinhole is logged on the same path, this archetype's components
will be ignored.

## Components

**Required**: [`DisconnectedSpace`](../components/disconnected_space.md)

## Links
 * 🐍 [Python API docs for `DisconnectedSpace`](https://ref.rerun.io/docs/python/HEAD/package/rerun/archetypes/disconnected_space/)
 * 🦀 [Rust API docs for `DisconnectedSpace`](https://docs.rs/rerun/0.9.0-alpha.6/rerun/archetypes/struct.DisconnectedSpace.html)

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

