---
title: "ViewCoordinates"
---
<!-- DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/docs/website.rs -->

⚠️ **This type is _unstable_ and may change significantly in a way that the data won't be backwards compatible.**
How we interpret the coordinate system of an entity/space.

For instance: What is "up"? What does the Z axis mean?

The three coordinates are always ordered as [x, y, z].

For example [Right, Down, Forward] means that the X axis points to the right, the Y axis points
down, and the Z axis points forward.

⚠ [Rerun does not yet support left-handed coordinate systems](https://github.com/rerun-io/rerun/issues/5032).

The following constants are used to represent the different directions:
 * Up = 1
 * Down = 2
 * Right = 3
 * Left = 4
 * Forward = 5
 * Back = 6


## Arrow datatype
```
FixedSizeList<3, uint8>
```

## API reference links
 * 🌊 [C++ API docs for `ViewCoordinates`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1datatypes_1_1ViewCoordinates.html)
 * 🐍 [Python API docs for `ViewCoordinates`](https://ref.rerun.io/docs/python/stable/common/datatypes#rerun.datatypes.ViewCoordinates)
 * 🦀 [Rust API docs for `ViewCoordinates`](https://docs.rs/rerun/latest/rerun/datatypes/struct.ViewCoordinates.html)


## Used by

* [`ViewCoordinates`](../components/view_coordinates.md)
