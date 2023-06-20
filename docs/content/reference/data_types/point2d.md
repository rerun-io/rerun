---
title: Point2D
order: 7
---
`Point2d` represents a singular point in two-dimensional space with optional color, radii, and label. `Point2d` entities will be drawn as part of the 2D Spatial SpaceView.

It is compatible with [`AnnotationContext`](../../concepts/annotation-context.md). `class_id` can be used to provide
colors and labels from the annotation context, and `keypoint_id` can be used to make connected edges between points. See
examples in the `AnnotationContext` documentation.

`draw_order` can be used to control how the `Point2d` entities are drawn relative to other objects within the scene. Higher values are drawn on top of lower values.

## Components and APIs

Primary component: `point2d`

Secondary components: `colorrgba`, `radius`, `label`, `class_id`, `keypoint_id`, `draw_order`

Python APIs: [log_point](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_point), [log_points](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_points)

Rust API: [Point2D](https://docs.rs/rerun/latest/rerun/components/struct.Point2D.html)

## Simple Example

code-example: point2d_simple

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/a5263f49955a41b24edf2fed6bd9dfe8398437d2_point2d_simple_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/2718de29f56f0340d16be71a053739198e4f3c6b_point2d_simple_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/ab64873e7154539f15ea967bfe9c842767c640e2_point2d_simple_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/5e9844b9f977bfcdff20ba751f2382f27dee6654_point2d_simple_1200w.png">
  <img  src="https://static.rerun.io/f0d2794efda38ec4c3c0337f1ee7c34b01e587f0_point2d_simple_full.png" alt="">
</picture>

## Full Example

code-example: point2d_random

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/cd7e4c3b892678869f6745db1d64715610d579f5_point2d_random_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/d0ef1083179354dc9a528ac4e18e6613142bc6d7_point2d_random_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/993efe1c06aea9d61c0dab1b271dc3c42795b03d_point2d_random_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/1585926a864514ae3a88e88341debf724fbed151_point2d_random_1200w.png">
  <img src="https://static.rerun.io/821426aa15139e417df2bd8854538666d11f8437_point2d_random_full.png" alt="">
</picture>
