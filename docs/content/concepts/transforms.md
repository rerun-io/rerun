---
title: Transforms & Transform Frames
order: 300
---

Rerun comes with built-in support for modelling spatial relationships between entities through.
This page details how the [different archetypes](https://rerun.io/docs/reference/types/archetypes#transforms) involved interact with each other and explains how geometric transforms are set up in Rerun.

## Transform hierarchies with entity paths

The [`Transform3D`](https://rerun.io/docs/reference/types/archetypes/transform3d) archetype allows you to specify how one coordinate system relates to another through translation, rotation, and scaling.

The simplest way to use transforms is through entity path hierarchies, where each transform describes the relationship between an entity and its parent path.
Note that by default, all entities are connected via an identity transforms (to opt out of that, you have to use explicit transform frames, more on that later).

TODO: make tested cross language snippet
```python
import rerun as rr

rr.init("transform_hierarchy_example", spawn=True)

# Log entities at their hierarchy positions
rr.log("sun", rr.Ellipsoids3D(centers=[0, 0, 0], half_sizes=[1, 1, 1], colors=[255, 200, 10]))
rr.log("sun/planet", rr.Ellipsoids3D(centers=[0, 0, 0], half_sizes=[0.4, 0.4, 0.4], colors=[40, 80, 200]))
rr.log("sun/planet/moon", rr.Ellipsoids3D(centers=[0, 0, 0], half_sizes=[0.15, 0.15, 0.15], colors=[180, 180, 180]))

# Define transforms - each describes the relationship to its parent
rr.log("sun/planet", rr.Transform3D(translation=[6.0, 0.0, 0.0]))  # Planet 6 units from sun
rr.log("sun/planet/moon", rr.Transform3D(translation=[3.0, 0.0, 0.0]))  # Moon 3 units from planet
```

In this hierarchy:
- The `sun` entity exists at the origin of its own coordinate system
- The `sun/planet` transform places the planet 6 units along x away from the sun
- The `sun/planet/moon` transform places the moon 3 units along x away from the planet

This creates a transform hierarchy where transforms propagate down the entity tree. The moon's final position in the sun's coordinate system is 9 units away (6 + 3),
because the transforms are applied sequentially.

## Explicit transform frames

While entity path hierarchies work well for many cases, sometimes you need more flexibility in organizing your transforms.
In particular for anyone familiar with ROS we recommend using explicit transform frames as it allows you to model
your data much closer to how it would be defined when using ROS' [tf2](https://wiki.ros.org/tf2) library.

In a nutshell, by explicitly specifying transform frames, you can decouple the spatial relationships from the entity hierarchy.

Instead of relying on the path relationships of entities, each entity is first associated with a named transform frame using
the [`CoordinateFrame`](https://rerun.io/docs/reference/types/archetypes/coordinate_frame) archetype.

The relationship between transform frames is then determined by logging [`Transform3D`](https://rerun.io/docs/reference/types/archetypes/transform3d)
with `child_frame` and `parent_frame` parameters to define the geometric relationship between two transform frames.

TODO: make tested cross language snippet
```python
import rerun as rr
import numpy as np

rr.init("explicit_frames_example", spawn=True)
rr.log("/", rr.ViewCoordinates.RIGHT_HAND_Z_UP, static=True)

# Define entities with explicit coordinate frames
rr.log("sun", rr.Ellipsoids3D(centers=[0, 0, 0], half_sizes=[1, 1, 1], colors=[255, 200, 10]),
       rr.CoordinateFrame("sun_frame"))
rr.log("planet", rr.Ellipsoids3D(centers=[0, 0, 0], half_sizes=[0.4, 0.4, 0.4], colors=[40, 80, 200]),
       rr.CoordinateFrame("planet_frame"))
rr.log("moon", rr.Ellipsoids3D(centers=[0, 0, 0], half_sizes=[0.15, 0.15, 0.15], colors=[180, 180, 180]),
       rr.CoordinateFrame("moon_frame"))

# Connect the viewer to the sun's coordinate frame
rr.log("/", rr.CoordinateFrame("sun_frame"))

# Define explicit frame relationships
rr.log("planet_transform", rr.Transform3D(
    translation=[6.0, 0.0, 0.0],
    child_frame="planet_frame",
    parent_frame="sun_frame"
))
rr.log("moon_transform", rr.Transform3D(
    translation=[3.0, 0.0, 0.0], 
    child_frame="moon_frame",
    parent_frame="planet_frame"
))
```

Note that unlike in ROS, you can log your transform relationship on _any_ entity.
However, currently once an entity specified the relation between two frames, this relation may no longer be logged on any other entity.

Named transform frames have a few of advantages over entity path based hierarchies:
* topology may change over time
* which entity is associated with which frame may change over time (it can also be [overridden via blueprint](..concepts/visualizers-and-overrides.md))
* several entities may be associated with the same frame without
* frees up entity paths for semantic rather than geometric organization

## Entity hierarchy based transforms under the hood - entity derived transform frames

Under the hood, Rerun's entity path hierarchies actually use the same transform frame system as named frames.
For each entity path, an associated transform frame with the prefix `tf#` automatically is automatically created:
for example, an entity `/world/robot` gets frame `tf#/world/robot`.

Path based hierarchies are then established by defaults the Viewer uses (also referred to as fallbacks):
Given an entity `/word/robot`:
* if no `CoordinateFrame::frame` is specified, it automatically defaults to `tf#/word/robot`
* if no `Transform3D::child_frame` is specified, it automatically defaults to `tf#/word/robot`
* if no `Transform3D::parent_frame` is specified, it automatically defaults to the parent's implicit frame, `tf#/word`

The only special properties these implicit frames have over their named counter parts is that they
have implicit identity relationships.

### Example

Given these entities:
TODO: xlanguage please
```python
rr.log("robot", rr.Transform3D(translation=[1, 0, 0]))
rr.log("robot/arm", rr.Transform3D(translation=[0, 1, 0]))
rr.log("robot/arm/gripper", rr.Points3D([0, 0, 0]))
```

Rerun will interpret this _as-if_ it was logged with the named transform frames like so:

TODO: xlanguage please
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

### Mixing explicit and implicit transform frames

We generally do not recommend mixing explicit and implicit transform frames since it can get confusing,
but doing so works seamlessly and can be useful in some situations.

Example:
TODO: xlanguage please.
```python
rr.log("robot", rr.Transform3D(translation=[1, 0, 0]))
rr.log("arm",
    rr.Transform3D(translation=[0, 1, 0], parent_frame="tf#/robot", child_frame="arm_frame"),
    rr.CoordinateFrame("arm_frame")
)
rr.log("gripper", rr.Points3D([0, 0, 0]), rr.CoordinateFrame("arm_frame"))
```

## Pinhole projections

In Rerun, pinhole cameras are also treated as spatial relationships that define projections from 3D spaces to 2D subspaces.
This unified approach allows the same transform system to handle both traditional 3D-to-3D transforms and 3D-to-2D projections seamlessly.

The [`Pinhole`](https://rerun.io/docs/reference/types/archetypes/pinhole) archetype defines this projection relationship through its intrinsic matrix (`image_from_camera`) and resolution.
Both implicit & named coordinate frames are supported, exactly as on [`Transform3D`](https://rerun.io/docs/reference/types/archetypes/transform3d).

With the right setup, pinholes allow a bunch of powerful visualizations:
* the pinhole glyph itself in 3D views
* 2D in 3D: all 2D content that is part of the pinhole's transform subtree
* 3D in 2D: if the pinhole is at the origin of the view, 3D objects can be projected through pinhole camera into the view.
    * Both the [nuscenes](https://rerun.io/examples/robotics/nuscenes_dataset) and [arkit](https://rerun.io/examples/spatial-computing/arkit_scenes) examples make use of this

### Example: 3D scene with 2D projections

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


## View coordinates

You can use the [`ViewCoordinates`](https://rerun.io/docs/reference/types/archetypes/view_coordinates) archetype to set your preferred view coordinate systems, giving semantic meaning to the XYZ axes of the space.

For 3D spaces it can be used to log what the up-axis is in your coordinate system. This will help Rerun set a good default view of your 3D scene, as well as make the virtual eye interactions more natural. In Python this can be done with `rr.log("/", rr.ViewCoordinates.RIGHT_HAND_Z_UP, static=True)`.
Note that in this example the archetype is logged at the root path, this will make it apply to all 3D views. Generally, a 3D view picks up view coordinates at or above its origin entity path.

[Pinholes](https://rerun.io/docs/reference/types/archetypes/view_coordinates) have a view coordinates field integrated as a shortcut.
The default coordinate system for pinhole entities is `RDF` (X=Right, Y=Down, Z=Forward).

WARNING: unlike in 3D views where `rr.ViewCoordinates` only impacts how the rendered scene is oriented, applying `rr.ViewCoordinates` to a pinhole-camera will actually influence the projection transform chain. Under the hood this value inserts a hidden transform that re-orients the axis of projection. Different world-content will be projected into your camera with different orientations depending on how you choose this value. See for instance the [`open_photogrammetry_format`](https://rerun.io/examples/3d-reconstruction/open_photogrammetry_format) example.

For 2D spaces and other entities, view coordinates currently have currently no effect ([#1387](https://github.com/rerun-io/rerun/issues/1387)).

## Poses & instancing

TODO: briefly explain poses, how they're relative to their entity's frame, how they can be used for instancing. Use a viewer embed of the instancing example.

## Visualizing transforms

TODO: write about how transforms can be visualized

## 2D Transforms

TODO: lack of 2d transforms
