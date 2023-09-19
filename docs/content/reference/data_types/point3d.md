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
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/03df4bf186a8a348443183f20bbcf81aa6466e85_point3d_simple_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/56461e6835eebdbe1f9f3cc12025ccc017dd85ee_point3d_simple_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/eea941cf7d29cd7a41bc8aaeebec4a358b623081_point3d_simple_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/94689c15c391b9ab7e8fbfefd695bf46a63f45a5_point3d_simple_1200w.png">
  <img src="https://static.rerun.io/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933_point3d_simple_full.png" alt="">
</picture>

## Full Example

code-example: point3d_random

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/c86336081c2a3209a831abfd8d873c970839a212_point3d_random_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/3099783f049ce15d512aa701c16286fb0a8ef6af_point3d_random_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/9f69a53273ac85e7c7e8841fca36301356a6293a_point3d_random_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/609d4a8d4a545f1bdcfe432c9403a6deac00d4a9_point3d_random_1200w.png">
  <img src="https://static.rerun.io/7e94e1806d2c381943748abbb3bedb68d564de24_point3d_random_full.png" alt="">
</picture>
