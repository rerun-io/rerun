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
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/point2d_simple/a8e801958bce5aa4e080659c033630f86ce95f71/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/point2d_simple/a8e801958bce5aa4e080659c033630f86ce95f71/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/point2d_simple/a8e801958bce5aa4e080659c033630f86ce95f71/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/point2d_simple/a8e801958bce5aa4e080659c033630f86ce95f71/1200w.png">
  <img src="https://static.rerun.io/point2d_simple/a8e801958bce5aa4e080659c033630f86ce95f71/full.png" alt="">
</picture>

## Full Example

code-example: point2d_random

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/point2d_random/8e8ac75373677bd72bd3f56a15e44fcab309a168/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/point2d_random/8e8ac75373677bd72bd3f56a15e44fcab309a168/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/point2d_random/8e8ac75373677bd72bd3f56a15e44fcab309a168/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/point2d_random/8e8ac75373677bd72bd3f56a15e44fcab309a168/1200w.png">
  <img src="https://static.rerun.io/point2d_random/8e8ac75373677bd72bd3f56a15e44fcab309a168/full.png" alt="">
</picture>
