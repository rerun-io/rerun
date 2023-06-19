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
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/f35cad24a53c62ddfff69b00ea93c5a728a7a1f7_box3d_simple_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/04d1eea2f304d804ff6340c329a38350bf87736f_box3d_simple_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/b7b2665bd4c973c795bafcb08a761574b06a164c_box3d_simple_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/365d39481497fa6eaad384ede680c012aea16d30_box3d_simple_1200w.png">
  <img src="https://static.rerun.io/4f45f6dc88bdaeb624ff1963fb858bc248c0efec_box3d_simple_full.png" alt="">
</picture>
