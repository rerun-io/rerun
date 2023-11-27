---
title: "PinholeProjection"
---

Camera projection, from image coordinates to view coordinates.

Child from parent.
Image coordinates from camera view coordinates.

Example:
```text
1496.1     0.0  980.5
   0.0  1496.1  744.5
   0.0     0.0    1.0
```

## Fields

* image_from_camera: [`Mat3x3`](../datatypes/mat3x3.md)

## Links
 * 🌊 [C++ API docs for `PinholeProjection`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1components_1_1PinholeProjection.html?speculative-link)
 * 🐍 [Python API docs for `PinholeProjection`](https://ref.rerun.io/docs/python/stable/common/components#rerun.components.PinholeProjection)
 * 🦀 [Rust API docs for `PinholeProjection`](https://docs.rs/rerun/latest/rerun/components/struct.PinholeProjection.html)


## Used by

* [`Pinhole`](../archetypes/pinhole.md)
