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
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/line_strip2d_simple/c4e6ce937544e66b497450fd64ac3ac2f244f0e1/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/line_strip2d_simple/c4e6ce937544e66b497450fd64ac3ac2f244f0e1/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/line_strip2d_simple/c4e6ce937544e66b497450fd64ac3ac2f244f0e1/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/line_strip2d_simple/c4e6ce937544e66b497450fd64ac3ac2f244f0e1/1200w.png">
  <img src="https://static.rerun.io/line_strip2d_simple/c4e6ce937544e66b497450fd64ac3ac2f244f0e1/full.png" alt="">
</picture>

code-example: line_segments2d_simple

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/line_segment2d_simple/53df596662dd9ffaaea5d09d091ef95220346c83/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/line_segment2d_simple/53df596662dd9ffaaea5d09d091ef95220346c83/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/line_segment2d_simple/53df596662dd9ffaaea5d09d091ef95220346c83/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/line_segment2d_simple/53df596662dd9ffaaea5d09d091ef95220346c83/1200w.png">
  <img src="https://static.rerun.io/line_segment2d_simple/53df596662dd9ffaaea5d09d091ef95220346c83/full.png" alt="">
</picture>

## Batch Examples

code-example: line_strip2d_batch

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/line_strip2d_batch/d8aae7ca3d6c3b0e3b636de60b8067fa2f0b6db9/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/line_strip2d_batch/d8aae7ca3d6c3b0e3b636de60b8067fa2f0b6db9/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/line_strip2d_batch/d8aae7ca3d6c3b0e3b636de60b8067fa2f0b6db9/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/line_strip2d_batch/d8aae7ca3d6c3b0e3b636de60b8067fa2f0b6db9/1200w.png">
  <img src="https://static.rerun.io/line_strip2d_batch/d8aae7ca3d6c3b0e3b636de60b8067fa2f0b6db9/full.png" alt="">
</picture>
