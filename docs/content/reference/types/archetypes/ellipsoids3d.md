---
title: "Ellipsoids3D"
---
<!-- DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/docs/mod.rs -->

3D ellipsoids or spheres.

This archetype is for ellipsoids or spheres whose size is a key part of the data
(e.g. a bounding sphere).
For points whose radii are for the sake of visualization, use [`archetypes.Points3D`](https://rerun.io/docs/reference/types/archetypes/points3d) instead.

Note that orienting and placing the ellipsoids/spheres is handled via `[archetypes.LeafTransforms3D]`.
Some of its component are repeated here for convenience.
If there's more leaf transforms than half sizes, the last half size will be repeated for the remaining transforms.

## Components

**Required**: [`HalfSize3D`](../components/half_size3d.md)

**Recommended**: [`LeafTranslation3D`](../components/leaf_translation3d.md), [`Color`](../components/color.md)

**Optional**: [`LeafRotationAxisAngle`](../components/leaf_rotation_axis_angle.md), [`LeafRotationQuat`](../components/leaf_rotation_quat.md), [`Radius`](../components/radius.md), [`FillMode`](../components/fill_mode.md), [`Text`](../components/text.md), [`ClassId`](../components/class_id.md)

## Shown in
* [Spatial3DView](../views/spatial3d_view.md)
* [Spatial2DView](../views/spatial2d_view.md) (if logged above active projection)

## API reference links
 * 🌊 [C++ API docs for `Ellipsoids3D`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1archetypes_1_1Ellipsoids3D.html?speculative-link)
 * 🐍 [Python API docs for `Ellipsoids3D`](https://ref.rerun.io/docs/python/stable/common/archetypes?speculative-link#rerun.archetypes.Ellipsoids3D)
 * 🦀 [Rust API docs for `Ellipsoids3D`](https://docs.rs/rerun/latest/rerun/archetypes/struct.Ellipsoids3D.html?speculative-link)

## Example

### Batch of ellipsoids

snippet: archetypes/ellipsoid_batch

