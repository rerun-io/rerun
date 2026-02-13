---
title: Transforms & Coordinate Frames
order: 400
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
**Note:** A current limitation to this is that once a `Transform3D` (or `Pinhole`) relating two frames has been logged to an entity, this particular relation may no longer be logged on any other entity.
An exception to this rule is [static data](static.md): if you log a frame to frame relationship on an entity with static time, you can later on use a different entity for temporal information.
This is useful to specify "default" transforms without yet knowing what timeline and paths are going to be used for temporal transforms.


Named transform frames have several advantages over entity path based hierarchies:
* topology may change over time
* association of entities with coordinate frames is explicit and may changed over time (it can also be [overridden via blueprint](../visualization/visualizers-and-overrides.md))
* several entities may be associated with the same frame
* frees up entity paths for semantic rather than geometric organization

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
rr.log("robot",
    rr.CoordinateFrame("tf#/robot"),
    rr.Transform3D(
        translation=[1, 0, 0],
        child_frame="tf#/robot",
        parent_frame="tf#/"
    )
)
rr.log("robot/arm",
    rr.CoordinateFrame("tf#/robot/arm"),
    rr.Transform3D(
        translation=[0, 1, 0],
        child_frame="tf#/robot/arm",
        parent_frame="tf#/robot"
    )
)
rr.log("robot/arm/gripper",
    rr.CoordinateFrame("tf#/robot/arm/gripper"),
    rr.Points3D([0, 0, 0])
)
```

<picture>
  <img src="https://static.rerun.io/transform_graph_translated/869b741ecce84c6b9af183922d32226a32a500bc/480w.png" alt="">
</picture>

#### Mixing named and implicit transform frames

We generally do not recommend mixing named and implicit transform frames since it can get confusing,
but doing so works seamlessly and can be useful if necessary.

Example:
```python
rr.log("robot", rr.Transform3D(translation=[1, 0, 0]))
rr.log("arm",
    rr.Transform3D(translation=[0, 1, 0], parent_frame="tf#/robot", child_frame="arm_frame"),
    rr.CoordinateFrame("arm_frame")
)
rr.log("gripper", rr.Points3D([0, 0, 0]), rr.CoordinateFrame("arm_frame"))
```

<picture>
  <img src="https://static.rerun.io/transform_graph_mixed/f01d4a4a5fd39b072dd439e93885e46d9e808825/480w.png" alt="">
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

If a transform frame relationship has both a pinhole projection & regular transforms (in this context often regarded as the camera extrinsics),
the regular transform is applied first.

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
  <img src="https://static.rerun.io/pinhole-projections/ceb1b4124e111b5d0a786dd48909a1cbb52eca4c/full.png">
</picture>


### View coordinates

You can use the [`ViewCoordinates`](https://rerun.io/docs/reference/types/archetypes/view_coordinates) archetype to set your preferred view coordinate systems, giving semantic meaning to the XYZ axes of the space.

For 3D spaces it can be used to log what the up-axis is in your coordinate system. This will help Rerun set a good default view of your 3D scene, as well as make the virtual eye interactions more natural. In Python this can be done with `rr.log("/", rr.ViewCoordinates.RIGHT_HAND_Z_UP, static=True)`.
Note that in this example the archetype is logged at the root path, this will make it apply to all 3D views. Generally, a 3D view picks up view coordinates at or above its origin entity path.

[Pinholes](https://rerun.io/docs/reference/types/archetypes/view_coordinates) have a view coordinates field integrated as a shortcut.
The default coordinate system for pinhole entities is `RDF` (X=Right, Y=Down, Z=Forward).

>  ⚠️ Unlike in 3D views where `rr.ViewCoordinates` only impacts how the rendered scene is oriented, applying `rr.ViewCoordinates` to a pinhole-camera will actually influence the projection transform chain. Under the hood this value inserts a hidden transform that re-orients the axis of projection. Different world-content will be projected into your camera with different orientations depending on how you choose this value. See for instance the [`open_photogrammetry_format`](https://rerun.io/examples/3d-reconstruction/open_photogrammetry_format) example.

For 2D spaces and other entities, view coordinates currently have currently no effect ([#1387](https://github.com/rerun-io/rerun/issues/1387)).

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
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/mesh3d_leaf_transforms3d/c2d0ee033129da53168f5705625a9b033f3a3d61/full.png">
  <img src="https://static.rerun.io/mesh3d_leaf_transforms3d/c2d0ee033129da53168f5705625a9b033f3a3d61/full.png">
</picture>

In this example, the mesh at `"shape"` is instantiated four times with different translations and rotations.
The box at `"shape/box"` is not affected by its parent's instance poses and appears only once.

<!--

Visualizing transforms

TODO(andreas, grtlr): write about how transforms can be visualized

2D Transforms

TODO(#349): lack of 2D transforms

-->
