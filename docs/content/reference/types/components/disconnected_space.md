---
title: "DisconnectedSpace"
---

Specifies that the entity path at which this is logged is spatially disconnected from its parent, making it impossible to transform the entity path into its parent's space and vice versa.

It *only* applies to space views that work with spatial transformations, i.e. 2D & 3D space views.
This is useful for specifying that a subgraph is independent of the rest of the scene.


## Links
 * 🌊 [C++ API docs for `DisconnectedSpace`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1components_1_1DisconnectedSpace.html)
 * 🐍 [Python API docs for `DisconnectedSpace`](https://ref.rerun.io/docs/python/stable/common/components#rerun.components.DisconnectedSpace)
 * 🦀 [Rust API docs for `DisconnectedSpace`](https://docs.rs/rerun/latest/rerun/components/struct.DisconnectedSpace.html)


## Used by

* [`DisconnectedSpace`](../archetypes/disconnected_space.md)
