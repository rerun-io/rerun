---
title: "Points2D"
---

A 2D point cloud with positions and optional colors, radii, labels, etc.

## Components

**Required**: [`Position2D`](../components/position2d.md)

**Recommended**: [`Radius`](../components/radius.md), [`Color`](../components/color.md)

**Optional**: [`Text`](../components/text.md), [`DrawOrder`](../components/draw_order.md), [`ClassId`](../components/class_id.md), [`KeypointId`](../components/keypoint_id.md), [`InstanceKey`](../components/instance_key.md)

## Links
 * 🌊 [C++ API docs for `Points2D`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1archetypes_1_1Points2D.html)
 * 🐍 [Python API docs for `Points2D`](https://ref.rerun.io/docs/python/stable/common/archetypes#rerun.archetypes.Points2D)
 * 🦀 [Rust API docs for `Points2D`](https://docs.rs/rerun/latest/rerun/archetypes/struct.Points2D.html)

## Examples

### Simple 2D points

code-example: point2d_simple

<center>
<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/point2d_simple/a8e801958bce5aa4e080659c033630f86ce95f71/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/point2d_simple/a8e801958bce5aa4e080659c033630f86ce95f71/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/point2d_simple/a8e801958bce5aa4e080659c033630f86ce95f71/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/point2d_simple/a8e801958bce5aa4e080659c033630f86ce95f71/1200w.png">
  <img src="https://static.rerun.io/point2d_simple/a8e801958bce5aa4e080659c033630f86ce95f71/full.png" width="640">
</picture>
</center>

### Randomly distributed 2D points with varying color and radius

code-example: point2d_random

<center>
<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/point2d_random/8e8ac75373677bd72bd3f56a15e44fcab309a168/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/point2d_random/8e8ac75373677bd72bd3f56a15e44fcab309a168/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/point2d_random/8e8ac75373677bd72bd3f56a15e44fcab309a168/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/point2d_random/8e8ac75373677bd72bd3f56a15e44fcab309a168/1200w.png">
  <img src="https://static.rerun.io/point2d_random/8e8ac75373677bd72bd3f56a15e44fcab309a168/full.png" width="640">
</picture>
</center>

