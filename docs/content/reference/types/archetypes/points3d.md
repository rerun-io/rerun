---
title: "Points3D"
---

A 3D point cloud with positions and optional colors, radii, labels, etc.

## Components

**Required**: [`Position3D`](../components/position3d.md)

**Recommended**: [`Radius`](../components/radius.md), [`Color`](../components/color.md)

**Optional**: [`Text`](../components/text.md), [`ClassId`](../components/class_id.md), [`KeypointId`](../components/keypoint_id.md), [`InstanceKey`](../components/instance_key.md)

## Links
 * 🌊 [C++ API docs for `Points3D`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1archetypes_1_1Points3D.html?speculative-link)
 * 🐍 [Python API docs for `Points3D`](https://ref.rerun.io/docs/python/stable/common/archetypes#rerun.archetypes.Points3D)
 * 🦀 [Rust API docs for `Points3D`](https://docs.rs/rerun/latest/rerun/archetypes/struct.Points3D.html)

## Examples

### Simple 3D points

code-example: point3d_simple

<center>
<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/1200w.png">
  <img src="https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/full.png" width="640">
</picture>
</center>

### Randomly distributed 3D points with varying color and radius

code-example: point3d_random

<center>
<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/1200w.png">
  <img src="https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/full.png" width="640">
</picture>
</center>

