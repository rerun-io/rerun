---
title: Linestrip2D
order: 4
---
`Linestrip2D` represents a series of connected line segments in two-dimensional space. The `linestrip2d` component is
defined by a list of 2d points, which are connected sequentially. Additionally, linestrips can be drawn with color and
radius. The radius controls the thickness of the line segments.

There are currently two python APIs that both use the same underlying `Linestrip2D` archetype.
 * [log_line_strip](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_line_strip) outputs a single linestrip from the provided points.
 * [log_line_segments](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_line_segments) outputs a batch of linestrips each made up of a single line.

`draw_order` can be used to control how the `Linestrip2D` entities are drawn relative to other objects within the scene.
Higher values are drawn on top of lower values.

Notes:
* There is not currently a python API for logging a batch of linestrips.
* In the python APIs `radius` is currently derived from `stroke_width`

## Components and APIs
Primary component: `linestrip2d`

Secondary components: `colorrgba`, `radius`, `draw_order`

Python APIs: [log_line_strip](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_line_strip), [log_line_segments](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_line_segments)

Rust API: [LineStrip2D](https://docs.rs/rerun/latest/rerun/components/struct.LineStrip2D.html)

## Simple Examples

code-example: line_strip2d_simple

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/53513e074a1d01388ac6ac9664ff9d452813870d_line_strip2d_simple_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/669c61837dc3464090945f6ade96f0205006e202_line_strip2d_simple_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/81cf822ebaa5faff8d129f2705872621835acc95_line_strip2d_simple_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/e549c9d69a19754803c648b665be5afbff9b7cad_line_strip2d_simple_1200w.png">
  <img src="https://static.rerun.io/c4e6ce937544e66b497450fd64ac3ac2f244f0e1_line_strip2d_simple_full.png" alt="">
</picture>

code-example: line_segments2d_simple

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/3c3604e215d461340ffc8dd53223406d732b44ac_line_segment2d_simple_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/7b0a8e60b0d6f005618c0ac09113ce84a08fb778_line_segment2d_simple_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/3ee262f61cb74aaffd7fd0b506754ee11cab3c12_line_segment2d_simple_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/196e0f2fe2222526e9eba87fa39440ada08e273d_line_segment2d_simple_1200w.png">
  <img src="https://static.rerun.io/53df596662dd9ffaaea5d09d091ef95220346c83_line_segment2d_simple_full.png" alt="">
</picture>

## Batch Examples

code-example: line_strip2d_batch

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/25e0ef495714636821fcd4dbf373148016bde195_line_strip2d_batch_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/b70bf5eefe036f08ed3e48cf4001cf4deebd86e6_line_strip2d_batch_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/44aaefeb430f8209c41df0c2cd4564538196b99d_line_strip2d_batch_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/e4cdfae79503362acd773bf1d124e95a1026b356_line_strip2d_batch_1200w.png">
  <img src="https://static.rerun.io/d8aae7ca3d6c3b0e3b636de60b8067fa2f0b6db9_line_strip2d_batch_full.png" alt="">
</picture>
