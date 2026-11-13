---
title: Transforms & Coordinate Frames
order: 800
description: Model spatial relationships with transforms, poses, and coordinate frames
---

<!-- Figma file for diagrams in this article: https://www.figma.com/board/PTwJKgi9kQOqG7ZgzdhrDL/Transforms-doc-page-graphs?t=fWkOGxxn6mZkkCON-1 -->

Rerun comes with built-in support for modeling spatial relationships between entities.
This page details how the [different archetypes](https://rerun.io/docs/reference/types/archetypes#transforms) involved interact with each other and explains how transforms are set up in Rerun.

## Transforms

### Entity path transforms

The [`Transform3D`](https://rerun.io/docs/reference/types/archetypes/transform3d) archetype allows you to specify how one coordinate system relates to another through translation, rotation, and scaling.

The simplest way to use transforms is through [entity path hierarchies](entity-path.md), where each transform describes the relationship between an entity and its parent path.
Note that by default, all entities are connected via identity transforms.

snippet: concepts/transform3d_hierarchy_simple

In this hierarchy:
- The `sun` entity exists at the origin of its own coordinate system
- The `sun/planet` transform places the planet 6 units from the sun, along the x-axis
- The `sun/planet/moon` transform places the moon 3 units along x away from the planet

This creates a transform hierarchy where transforms propagate down the entity tree. The moon's final position in the sun's coordinate system is 9 units away (6 + 3),
because the transforms are applied sequentially.

### Named transform frames

While entity path hierarchies work well for many cases, sometimes you need more flexibility in organizing your transforms.
In particular, for anyone familiar with ROS, we recommend using named transform frames as it allows you to model
your data much closer to how it would be defined when using ROS' [tf2](https://wiki.ros.org/tf2) library.

By explicitly specifying transform frames, you can decouple spatial relationships from the entity hierarchy.

Instead of relying on entity path relationships, each entity is first associated with a named transform frame using
the [`CoordinateFrame`](https://rerun.io/docs/reference/types/archetypes/coordinate_frame) archetype.

The geometric relationship between two transform frames is then determined by logging [`Transform3D`](https://rerun.io/docs/reference/types/archetypes/transform3d)
with `child_frame` and `parent_frame` parameters set to their respective names.

snippet: concepts/transform3d_hierarchy_named_frames

Note that unlike in ROS, you can log your transform relationship on _any_ entity.

> [!NOTE]
> A current limitation to this is that once a `Transform3D` (or `Pinhole`) relating two frames has been logged to an entity, this particular relation may no longer be logged on any other entity.
> An exception to this rule is [static data](static.md): if you log a frame to frame relationship on an entity with static time, you can later on use a different entity for temporal information.
> This is useful to specify "default" transforms without yet knowing what timeline and paths are going to be used for temporal transforms.


Named transform frames have several advantages over entity path based hierarchies:
* topology may change over time
* association of entities with coordinate frames is explicit and may change over time (it can also be [overridden via blueprint](../../viewer/blueprints-and-views/customize-views.md))
* several entities may be associated with the same frame
* frees up entity paths for semantic rather than geometric organization

### One parent per frame

A transform frame has exactly one parent at any point in time. This is the same convention that ROS' [tf2](https://wiki.ros.org/tf2) uses.

A transform relationship is identified by its `child_frame`.
Logging another `Transform3D` with the same `child_frame` but a different `parent_frame` therefore does not add a second relationship, it replaces the existing one:

```python
rr.log("tf", rr.Transform3D(parent_frame="first_parent", child_frame="child"))
rr.log("tf", rr.Transform3D(parent_frame="second_parent", child_frame="child"))
```

Both calls are part of the recording. `child` has `first_parent` as its parent up to the second call, and `second_parent` from there on.
Since transforms are resolved for a single point in time, `first_parent` and `child` end up unrelated as soon as the time cursor is past the second call.

This structure can make expressing a transform with two natural parents a bit unintuitive. Typically, it is best to invert one of the two relationships instead and use `relation=ChildFromParent`:

```python
rr.log("tf", rr.Transform3D(parent_frame="first_parent", child_frame="child"))
# Same values as the `second_parent` to `child` transform, expressed as `child` to `second_parent`:
rr.log(
    "tf",
    rr.Transform3D(parent_frame="child", child_frame="second_parent", relation=rr.TransformRelation.ChildFromParent),
)
```

`first_parent` now stays connected and all three frames can be transformed into each other.

### Entity hierarchy based transforms under the hood

Under the hood, Rerun's entity path hierarchies actually use the same transform frame system as named frames.
For each entity path, an associated transform frame with the prefix `tf#` is automatically created:
for example, an entity `/world/robot` gets frame `tf#/world/robot`.

Path based hierarchies are then established by defaults the Viewer uses (also referred to as fallbacks):
Given an entity `/world/robot`:
* if no `CoordinateFrame::frame` is specified, it automatically defaults to `tf#/world/robot`
* if no `Transform3D::child_frame` is specified, it automatically defaults to `tf#/world/robot`
* if no `Transform3D::parent_frame` is specified, it automatically defaults to the parent's implicit frame, `tf#/world`

The only special properties these implicit frames have over their named counterparts is that they
have implicit identity relationships.

#### Example

Given these entities:
```python
rr.log("robot", rr.Transform3D(translation=[1, 0, 0]))
rr.log("robot/arm", rr.Transform3D(translation=[0, 1, 0]))
rr.log("robot/arm/gripper", rr.Points3D([0, 0, 0]))
```

Rerun will interpret this _as-if_ it was logged with the named transform frames like so:

```python
rr.log(
    "robot",
    rr.CoordinateFrame("tf#/robot"),
    rr.Transform3D(translation=[1, 0, 0], child_frame="tf#/robot", parent_frame="tf#/"),
)
rr.log(
    "robot/arm",
    rr.CoordinateFrame("tf#/robot/arm"),
    rr.Transform3D(translation=[0, 1, 0], child_frame="tf#/robot/arm", parent_frame="tf#/robot"),
)
rr.log("robot/arm/gripper", rr.CoordinateFrame("tf#/robot/arm/gripper"), rr.Points3D([0, 0, 0]))
```

<picture>
  <img src="https://static.rerun.io/transform_graph_translated/869b741ecce84c6b9af183922d32226a32a500bc/480w.png" alt="Diagram linking entities /robot, /robot/arm and /robot/arm/gripper to implicit tf# frames chained from child to parent">
</picture>

#### Mixing named and implicit transform frames

We generally do not recommend mixing named and implicit transform frames since it can get confusing,
but doing so works seamlessly and can be useful if necessary.

Example:
```python
rr.log("robot", rr.Transform3D(translation=[1, 0, 0]))
rr.log(
    "arm",
    rr.Transform3D(translation=[0, 1, 0], parent_frame="tf#/robot", child_frame="arm_frame"),
    rr.CoordinateFrame("arm_frame"),
)
rr.log("gripper", rr.Points3D([0, 0, 0]), rr.CoordinateFrame("arm_frame"))
```

<picture>
  <img src="https://static.rerun.io/transform_graph_mixed/f01d4a4a5fd39b072dd439e93885e46d9e808825/480w.png" alt="Diagram of /robot linked to implicit frame tf#/robot, with /arm and /gripper both linked to arm_frame, a child of tf#/robot">
</picture>

## Other transform types

### Pinhole projections

In Rerun, pinhole cameras are not merely another archetype that can be visualized,
they are also treated as spatial relationships that define projections from 3D spaces to 2D subspaces.
This unified approach allows the Viewer to handle both traditional 3D-to-3D transforms and 3D-to-2D projections.

The [`Pinhole`](https://rerun.io/docs/reference/types/archetypes/pinhole) archetype defines this projection relationship through its intrinsic matrix (`image_from_camera`) and resolution.
Both implicit & named coordinate frames are supported, exactly as on [`Transform3D`](https://rerun.io/docs/reference/types/archetypes/transform3d).

With the right setup, pinholes allow a bunch of powerful visualizations:
* the pinhole glyph itself in 3D views
* 2D in 3D: all 2D content that is part of the pinhole's transform subtree
* 3D in 2D: if the pinhole is at the origin of the view, 3D objects can be projected through pinhole camera into the view.
    * Both the [nuscenes](https://rerun.io/examples/robotics/nuscenes_dataset) and [arkit](https://rerun.io/examples/spatial-computing/arkit_scenes) examples make use of this

A transform frame relationship can have both a `Pinhole` and a regular `Transform3D` (in this context often regarded as the camera extrinsics).
Both are used: the `Transform3D` places the camera in the parent space, and the `Pinhole` projects from the camera's 3D space onto the 2D image.
So when going from the parent 3D space to the image, the regular transform is applied first, then the projection.

#### Sizing the frustum

The `Pinhole` archetype has three fields that only change how the camera looks in 3D views:

- `image_plane_distance`: how far from the camera origin the image plane (and thus the frustum) is drawn.
  It does not affect the projection.
  By default, it is derived from the size of the scene.
- `color`: the color of the frustum wireframe.
- `line_width`: the width of the frustum wireframe lines.

```python
rr.log(
    "world/camera",
    rr.Pinhole(focal_length=500, width=640, height=480, image_plane_distance=0.5, color=[255, 128, 0], line_width=0.01),
)
```

You can also set these for all cameras in a view with a [view default](../../viewer/blueprints-and-views/customize-views.md#per-view-component-default), e.g. `rrb.Spatial3DView(defaults=[rr.Pinhole.from_fields(image_plane_distance=0.5)])`.

A [`DepthImage`](../../reference/types/archetypes/depth_image.md) logged under a pinhole entity is back-projected into a 3D point cloud in 3D views, using the pinhole's intrinsics.

#### Example: 3D scene with 2D projections

Here's how to set up a 3D scene with pinhole cameras that create 2D projections:

In this example, the 3D objects (box and points) are automatically projected into the 2D camera view,
demonstrating how Rerun's transform system handles the spatial relationship between 3D world coordinates
and 2D image coordinates through pinhole projections.

snippet: archetypes/pinhole_projections

<picture data-inline-viewer="snippets/archetypes/pinhole_projections">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/pinhole-projections/ceb1b4124e111b5d0a786dd48909a1cbb52eca4c/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/pinhole-projections/ceb1b4124e111b5d0a786dd48909a1cbb52eca4c/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/pinhole-projections/ceb1b4124e111b5d0a786dd48909a1cbb52eca4c/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/pinhole-projections/ceb1b4124e111b5d0a786dd48909a1cbb52eca4c/1200w.png">
  <img src="https://static.rerun.io/pinhole-projections/ceb1b4124e111b5d0a786dd48909a1cbb52eca4c/full.png" alt="3D view of colored spheres, a red wireframe box and a checkerboard pinhole camera, beside the 2D camera view of their projection">
</picture>


### View coordinates

[`ViewCoordinates`](https://rerun.io/docs/reference/types/archetypes/view_coordinates) describes an orientation convention by specifying the directions in which the positive X, Y, and Z axes point.
It controls camera and view orientation.

For a 3D view, [`SpatialInformation.axes`](https://rerun.io/docs/reference/types/views/spatial3d_view) controls the eye orientation, navigation, and default grid plane.
A logged `ViewCoordinates` archetype provides the default value for 3D views whose origin is at or below that entity.
The view uses the value at its origin or the closest ancestor, so logging `rr.log("/", rr.ViewCoordinates.RIGHT_HAND_Z_UP, static=True)` applies to all 3D views unless a view overrides it in its blueprint.
If no archetype value is found, the view uses `camera_xyz` from the closest ancestor pinhole, followed by `RFU` as the final fallback.
This orientation does not change logged transforms.

For a [`Pinhole`](https://rerun.io/docs/reference/types/archetypes/pinhole), `camera_xyz` controls the camera orientation and projection direction.
The default is `RDF`: +X is right, +Y is down, and +Z is forward.
Changing `camera_xyz` reorients the camera frustum, projected depth data, and the projection of 3D content into the camera view.
See the [`open_photogrammetry_format`](https://rerun.io/examples/3d-reconstruction/open_photogrammetry_format) example for a non-default camera orientation.

For 2D spaces and other entities, view coordinates currently have no effect ([#1387](https://github.com/rerun-io/rerun/issues/1387)).

### Pose transforms

[`InstancePoses3D`](https://rerun.io/docs/reference/types/archetypes/instance_poses3d) defines geometric poses relative to an entity's transform frame.
Unlike with [`Transform3D`](https://rerun.io/docs/reference/types/archetypes/transform3d), poses do not propagate through the transform hierarchy
and can store an arbitrary amount of transforms on the same entity.

For an entity that has both [`Transform3D`](https://rerun.io/docs/reference/types/archetypes/transform3d)
(without `child_frame`/`parent_frame`) and `InstancePoses3D`,
the [`Transform3D`](https://rerun.io/docs/reference/types/archetypes/transform3d) is applied first
(affecting the entity and all its children), then [`InstancePoses3D`](https://rerun.io/docs/reference/types/archetypes/instance_poses3d)
is applied only to that specific entity.
(This is consistent with how entity hierarchy based transforms translate to transform frames.)

#### Instancing

Rerun's [`InstancePoses3D`](https://rerun.io/docs/reference/types/archetypes/instance_poses3d) archetype is not only used
to model poses relative to an Entity's frame, but also for repeating (known as "instancing") visualizations on the same entity:
most visualizations will show once for each transform on [`InstancePoses3D`](https://rerun.io/docs/reference/types/archetypes/instance_poses3d)
in the respective place.

snippet: archetypes/mesh3d_instancing

<picture data-inline-viewer="snippets/archetypes/mesh3d_instancing">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/mesh3d_leaf_transforms3d/c2d0ee033129da53168f5705625a9b033f3a3d61/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/mesh3d_leaf_transforms3d/c2d0ee033129da53168f5705625a9b033f3a3d61/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/mesh3d_leaf_transforms3d/c2d0ee033129da53168f5705625a9b033f3a3d61/1024w.png">
  <img src="https://static.rerun.io/mesh3d_leaf_transforms3d/c2d0ee033129da53168f5705625a9b033f3a3d61/full.png" alt="3D view of a rainbow-colored pyramid mesh instanced four times inside a single magenta wireframe box">
</picture>

In this example, the mesh at `"shape"` is instantiated four times with different translations and rotations.
The box at `"shape/box"` is not affected by its parent's instance poses and appears only once.

## Rotation conventions and precision

- **Quaternions are `xyzw`**, with the scalar last: `rr.Quaternion(xyzw=[x, y, z, w])`.
  Libraries such as Eigen or ROS messages may store them in a different order, so check before passing them on.
- **There are no Euler angles.**
  Convert them yourself, e.g. with `scipy.spatial.transform.Rotation.from_euler("xyz", angles).as_quat()` (which also returns `xyzw`).
  You can also use `rr.RotationAxisAngle(axis=…, degrees=…)` or a 3×3 matrix (`mat3x3`).
- **Order of operations:** the components of `Transform3D` are applied in the reverse order they are listed: `mat3x3` first, then `scale`, then the rotation, then `translation`.
- **Transforms are 32-bit floats.**
  All transform values (translation, rotation, scale, and matrices) are stored as `Float32`, so 64-bit values are converted when logged.
  Very large coordinates, such as GPS or UTM positions, lose precision and can make the scene jitter.
  Subtract a local origin before logging so the values stay small.
  64-bit transforms are [not supported yet](../../development/roadmap.md#missing-features).

## Visualizing transforms

In 3D views, a transform can be shown as three axis arrows (red, green, and blue for X, Y, and Z).

- The arrows are shown when the entity has a [`TransformAxes3D`](../../reference/types/archetypes/transform_axes3d.md) archetype, and automatically for entities with a `Pinhole`.
  A `Transform3D` alone shows no arrows.
  To show them, log `TransformAxes3D` on the same entity:

  ```python
  rr.log("robot/arm", rr.Transform3D(translation=[0, 1, 0]), rr.TransformAxes3D(axis_length=0.5))
  ```

- `axis_length` sets the arrow length, in the local units of the transform.
  You can also set it with a blueprint override or [view default](../../viewer/blueprints-and-views/customize-views.md#per-view-component-default).
- To hide the arrows, select the entity and remove the `TransformAxes3D` visualizer in the selection panel.
- To show axes at the origin of the 3D view itself, set `show_axes` on the view: `rrb.Spatial3DView(spatial_information=rrb.SpatialInformation(show_axes=True))`.

## Reading transforms back

There is no SDK API that resolves the final pose of an entity in the world (the composition of all transforms up the tree).

You can:

- Query the logged `Transform3D` (and `Pinhole`) columns with the [dataframe API](../../query-and-transform/catalog/get-data-out.md), and compose the transforms yourself.
- Ask a running Viewer with the experimental `rerun.experimental.ViewerClient.get_transform_graph()`.
  It returns the frames and the latest _direct_ parent-child edges at the Viewer's current time, not composed world poses.
  Pass `include_transforms=True` to get the transform values.

## Debugging transform trees

The Rerun viewer features a debugging UI for visual introspection of the 3D transform cache.
This allows to view the tree structure of the transforms, including potentially disconnected trees, and inspect the latest stored values of each frame node or transform edge.
The UI supports horizontal and vertical tree layout and you can filter by transform type (e.g. static or temporal).

> [!NOTE]
> This UI is currently a tab in the dev panel (accessible via "Toggle dev panel" in the menu or ctrl/cmd+shift+m).
> We are open to making this a dedicated view in the future - let us know if you have any [feedback](https://rerun.io/docs/development)!

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/cc6c41138eeeabb31fb2ec988eefdcd8da446c86_transform_dev_panel_teaser.mp4" type="video/mp4" />
</video>

Agents can also inspect the transform graph through the [Viewer's MCP server](../../viewer/extend/mcp.md#inspecting-transforms) using `rerun_get_transform_graph`.
The tool provides a machine-readable JSON snapshot of the transform graph, including frame connections and optional transform values.

<!--

2D Transforms

TODO(#349): lack of 2D transforms

-->
