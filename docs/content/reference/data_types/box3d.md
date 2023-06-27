---
title: Box3D
order: 3
---
`Box3D` represents an oriented bounding box in three-dimensional space. The `box3d` component is defined by the
half-widths of the three box dimensions. By default the box will be centered at the origin and aligned with the axes.
The box can be positioned within it's local [space](../../concepts/spaces-and-transforms.md) by providing the `vec3d` position, or a `quaternion` orientation.

It is compatible with [`AnnotationContext`](../../concepts/annotation-context.md). `class_id` can be used to provide
colors and labels from the annotation context. See examples in the
[`AnnotationContext`](../../concepts/annotation-context.md) documentation.

## Components and APIs
Primary component: `box3d`,

Secondary components: `vec3d`, `quaternion`, `colorrgba`, `radius`, `label`, `classid`

Python APIs: [log_obb](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_obb)

Rust API: [Box3D](https://docs.rs/rerun/latest/rerun/components/struct.Box3D.html)

## Simple Example

code-example: box3d_simple

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/1342a41030eaddbe43439951076723298218e922_box3d_simple_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/05ed697e151dcb53a8a17dfac1bec2023e096083_box3d_simple_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/f47370115cb45b1085f87824d00ab38f95960732_box3d_simple_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/d8df3a0a665b4f5b034883684d73d767fcde6eef_box3d_simple_1200w.png">
  <img src="https://static.rerun.io/d6a3f38d2e3360fbacac52bb43e44762635be9c8_box3d_simple_full.png" alt="">
</picture>
