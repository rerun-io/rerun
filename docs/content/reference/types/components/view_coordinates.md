---
title: "ViewCoordinates"
---

How we interpret the coordinate system of an entity/space.

For instance: What is "up"? What does the Z axis mean? Is this right-handed or left-handed?

The three coordinates are always ordered as [x, y, z].

For example [Right, Down, Forward] means that the X axis points to the right, the Y axis points
down, and the Z axis points forward.

The following constants are used to represent the different directions:
 * Up = 1
 * Down = 2
 * Right = 3
 * Left = 4
 * Forward = 5
 * Back = 6


## Links
 * 🌊 [C++ API docs for `ViewCoordinates`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1components_1_1ViewCoordinates.html?speculative-link)
 * 🐍 [Python API docs for `ViewCoordinates`](https://ref.rerun.io/docs/python/stable/common/components#rerun.components.ViewCoordinates)
 * 🦀 [Rust API docs for `ViewCoordinates`](https://docs.rs/rerun/latest/rerun/components/struct.ViewCoordinates.html)


## Used by

* [`Pinhole`](../archetypes/pinhole.md)
* [`ViewCoordinates`](../archetypes/view_coordinates.md)
