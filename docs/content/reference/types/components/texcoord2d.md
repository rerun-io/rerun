---
title: "Texcoord2D"
---

A 2D texture UV coordinate.

Texture coordinates specify a position on a 2D texture.
A range from 0-1 in covers the entire texture in the respective dimension.
The behavior for values outside of this range depends on the visualization/renderer,
but will most commonly repeat the texture.
Rerun uses top-left as the origin for UV coordinates.

  0     U     1
0 + --------- →
  |           .
V |           .
  |           .
1 ↓ . . . . . .

This is the same convention as in Vulkan/Metal/DX12/WebGPU, but (!) unlike OpenGL,
which places the origin at the bottom-left.

## Fields

* uv: [`Vec2D`](../datatypes/vec2d.md)

## Links
 * 🌊 [C++ API docs for `Texcoord2D`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1components_1_1Texcoord2D.html)
 * 🐍 [Python API docs for `Texcoord2D`](https://ref.rerun.io/docs/python/stable/common/components#rerun.components.Texcoord2D)
 * 🦀 [Rust API docs for `Texcoord2D`](https://docs.rs/rerun/latest/rerun/components/struct.Texcoord2D.html)


## Used by

* [`Mesh3D`](../archetypes/mesh3d.md)
