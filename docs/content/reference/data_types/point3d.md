---
title: Point3D
order: 8
---
`Point3D` represents a singular point in three-dimensional space with optional color, radii, and label. `Point3D` entities will be drawn as part of the 3D Spatial SpaceView.

It is compatible with [`AnnotationContext`](../../concepts/annotation-context.md). `class_id` can be used to provide
colors and labels from the annotation context, and `keypoint_id` can be used to make connected edges between points. See
examples in the `AnnotationContext` documentation.

## Components and APIs
Primary component: `point3d`

Secondary components: `colorrgba`, `radius`, `label`, `classid`, `keypointid`

Python APIs: [log_point](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_point), [log_points](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_points)

Rust API: [Point3D](https://docs.rs/rerun/latest/rerun/components/struct.Point3D.html)

## Simple Example
code-example: point3d_simple

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/1200w.png">
  <img src="https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/full.png" alt="">
</picture>

## Full Example

code-example: point3d_random

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/1200w.png">
  <img src="https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/full.png" alt="">
</picture>
