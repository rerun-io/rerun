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
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/f82ec6e1ba1fa95fe7425bf1ef74657fada8fd6e_point3d_simple_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/2a9c68cc1367122352d0f40e763621fbfc329c2e_point3d_simple_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/8f07e8d2f7f06d0d44b6cc5f317838dbab537be3_point3d_simple_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/0e0cb320f93107f160b5fe0bd8d0fb107e4d18f7_point3d_simple_1200w.png">
  <img style="width: 75%;" src="https://static.rerun.io/8b6cd38dbc7cb06a1be1ccae52f10333db88ecd9_point3d_simple_full.png" alt="">
</picture>

## Full Example

code-example: point3d_random

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/b613ced38a3a6c0d8c9eda92853790383a85fc1b_point3d_random_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/9f38d658433ed672668e2130b5b61f7f3a340868_point3d_random_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/a53495732a85d2589ef78457aa3f7733af168044_point3d_random_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/b6c6c4f7bb6eee40b3e8be5bd41aff84ade519f5_point3d_random_1200w.png">
  <img style="width: 75%;" src="https://static.rerun.io/87744fc98ed6f59460692ac1f719a202119fdc37_point3d_random_full.png" alt="">
</picture>
