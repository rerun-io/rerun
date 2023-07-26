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
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/fd5b1ed1c42315ff5b1c2d1245ad655e4564d5f1_line_strip3d_simple_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/097f3c5086790e36e0b8a7f34a44e9eeac5227d2_line_strip3d_simple_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/ec76e1078e443fd4e71b751f84fe19f5b014272b_line_strip3d_simple_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/66b12e078e3a08d7a61f3f91f7ad847cbb4933dd_line_strip3d_simple_1200w.png">
  <img src="https://static.rerun.io/13036c0e71f78d3cec37d5724f97b47c4cf3c429_line_strip3d_simple_full.png" alt="">
</picture>

code-example: line_segments3d_simple

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/83b63186b05794227010dc1d083161add5ec7f0b_line_segment3d_simple_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/cb005f7b4e629e9b88a91835edfa066101f94f65_line_segment3d_simple_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/140ddb2aa68b3e1a623cd1997df808e255a2136c_line_segment3d_simple_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/aeed681be95d6c974446f89a6fa26b7d3077adce_line_segment3d_simple_1200w.png">
  <img src="https://static.rerun.io/aa800b2a6e6a7b8e32e762b42861bae36f5014bb_line_segment3d_simple_full.png" alt="">
</picture>

## Batch Examples

code-example: line_strip3d_batch

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/447c7d3d0a75447aa9bad9cfb2c6d68fbe082935_line_strip3d_batch_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/d7a16841654a524521f0d26b81771d4e5a740108_line_strip3d_batch_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/2848a42a715b410f433a9b78ddbe599dea2b66f9_line_strip3d_batch_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/a5ccbe907ea07baeb5117b17dfde41ce11477bf1_line_strip3d_batch_1200w.png">
  <img src="https://static.rerun.io/102e5ec5271475657fbc76b469267e4ec8e84337_line_strip3d_batch_full.png" alt="">
</picture>
