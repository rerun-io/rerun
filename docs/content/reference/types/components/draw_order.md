---
title: "DrawOrder"
---

Draw order used for the display order of 2D elements.

Higher values are drawn on top of lower values.
An entity can have only a single draw order component.
Within an entity draw order is governed by the order of the components.

Draw order for entities with the same draw order is generally undefined.


## Links
 * 🐍 [Python API docs for `DrawOrder`](https://ref.rerun.io/docs/python/stable/common/components#rerun.components.DrawOrder)
 * 🦀 [Rust API docs for `DrawOrder`](https://docs.rs/rerun/latest/rerun/components/struct.DrawOrder.html)


## Used by

* [`Boxes2D`](../archetypes/boxes2d.md)
* [`DepthImage`](../archetypes/depth_image.md)
* [`Image`](../archetypes/image.md)
* [`LineStrips2D`](../archetypes/line_strips2d.md)
* [`Points2D`](../archetypes/points2d.md)
* [`SegmentationImage`](../archetypes/segmentation_image.md)
