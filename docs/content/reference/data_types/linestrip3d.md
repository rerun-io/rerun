---
title: Linestrip3D
order: 5
---
`Linestrip3D` represents a series of connected line segments in three-dimensional space. The `linestrip3d` component is
defined by a list of 3d points, which are connected sequentially. Additionally, linestrips can be drawn with color and
radius. The radius controls the thickness of the line segments.

There are currently two python APIs that both use the same underlying `Linestrip3D` archetype.
 * [log_line_strip](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_line_strip) outputs a single linestrip from the provided points.
 * [log_line_segments](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_line_segments) outputs a batch of linestrips each made up of a single line.

Notes:
* There is not currently a python API for logging a batch of linestrips.
* In the python APIs `radius` is currently derived from `stroke_width`

## Components and APIs
Primary component: `linestrip3d`

Secondary components: `colorrgba`, `radius`

Python APIs: [log_line_strip](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_line_strip), [log_line_segments](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_line_segments)

Rust API: [LineStrip3D](https://docs.rs/rerun/latest/rerun/components/struct.LineStrip3D.html)

## Simple Examples

code-example: line_strip3d_simple

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/line_strip3d_simple/13036c0e71f78d3cec37d5724f97b47c4cf3c429/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/line_strip3d_simple/13036c0e71f78d3cec37d5724f97b47c4cf3c429/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/line_strip3d_simple/13036c0e71f78d3cec37d5724f97b47c4cf3c429/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/line_strip3d_simple/13036c0e71f78d3cec37d5724f97b47c4cf3c429/1200w.png">
  <img src="https://static.rerun.io/line_strip3d_simple/13036c0e71f78d3cec37d5724f97b47c4cf3c429/full.png" alt="">
</picture>

code-example: line_segments3d_simple

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/line_segment3d_simple/aa800b2a6e6a7b8e32e762b42861bae36f5014bb/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/line_segment3d_simple/aa800b2a6e6a7b8e32e762b42861bae36f5014bb/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/line_segment3d_simple/aa800b2a6e6a7b8e32e762b42861bae36f5014bb/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/line_segment3d_simple/aa800b2a6e6a7b8e32e762b42861bae36f5014bb/1200w.png">
  <img src="https://static.rerun.io/line_segment3d_simple/aa800b2a6e6a7b8e32e762b42861bae36f5014bb/full.png" alt="">
</picture>

## Batch Examples

code-example: line_strip3d_batch

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/line_strip3d_batch/102e5ec5271475657fbc76b469267e4ec8e84337/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/line_strip3d_batch/102e5ec5271475657fbc76b469267e4ec8e84337/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/line_strip3d_batch/102e5ec5271475657fbc76b469267e4ec8e84337/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/line_strip3d_batch/102e5ec5271475657fbc76b469267e4ec8e84337/1200w.png">
  <img src="https://static.rerun.io/line_strip3d_batch/102e5ec5271475657fbc76b469267e4ec8e84337/full.png" alt="">
</picture>
