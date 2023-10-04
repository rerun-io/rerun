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
 * 🐍 Python API docs: https://ref.rerun.io/docs/python/HEAD/package/rerun/components/pinhole_projection/
 * 🦀 Rust API docs: https://docs.rs/rerun/0.9.0-alpha.6/rerun/components/struct.PinholeProjection.html


## Used by

* [`Pinhole`](../archetypes/pinhole.md)
