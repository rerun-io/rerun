---
title: "ViewCoordinates"
---

How we interpret the coordinate system of an entity/space.

For instance: What is "up"? What does the Z axis mean? Is this right-handed or left-handed?

The three coordinates are always ordered as [x, y, z].

For example [Right, Down, Forward] means that the X axis points to the right, the Y axis points
down, and the Z axis points forward.

## Components

**Required**: [`ViewCoordinates`](../components/view_coordinates.md)

## Links
 * 🌊 [C++ API docs for `ViewCoordinates`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1archetypes_1_1ViewCoordinates.html)
 * 🐍 [Python API docs for `ViewCoordinates`](https://ref.rerun.io/docs/python/stable/common/archetypes#rerun.archetypes.ViewCoordinates)
 * 🦀 [Rust API docs for `ViewCoordinates`](https://docs.rs/rerun/latest/rerun/archetypes/struct.ViewCoordinates.html)

## Example

### View coordinates for adjusting the eye camera

code-example: view_coordinates_simple

<center>
<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viewcoordinates/0833f0dc8616a676b7b2c566f2a6f613363680c5/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viewcoordinates/0833f0dc8616a676b7b2c566f2a6f613363680c5/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viewcoordinates/0833f0dc8616a676b7b2c566f2a6f613363680c5/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viewcoordinates/0833f0dc8616a676b7b2c566f2a6f613363680c5/1200w.png">
  <img src="https://static.rerun.io/viewcoordinates/0833f0dc8616a676b7b2c566f2a6f613363680c5/full.png" width="640">
</picture>
</center>

