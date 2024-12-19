---
title: Spaces and Transforms
order: 300
---

## The definition of a space

Every entity in Rerun exists in some _space_. This is at the core of how Rerun organizes the visualizations of the data
that you have logged. In the [Rerun Viewer](../reference/viewer.md) you view data by configuring a _view_, which is a view
of a set of entities _as seen from a particular origin._

The origin of a space is, very loosely, a generalization of the idea of a "coordinate system" (sometimes known as a "coordinate frame") to arbitrary data. If a collection of
entities are part of the same space, it means they can be rendered together.

For example:

-   For 2D and 3D geometric primitives this means they share the same coordinate system.
-   For scalar plots it means they share the same plot axes.
-   For text logs, it means they share the same conceptual stream.

As explained below, a view _may_ display data belonging to multiple spaces, but there must be a well-defined
means of transforming the data from one space to another.

Which entities belong to which spaces is a function of the transform system, which uses the following rules to define
the space connectivity:

1. Every unique entity path defines a potentially unique space.
1. Unless otherwise specified, every path is trivially connected to its parent by the identity transform.
1. Logging a transform to a path defines the relationship between that path and its parent (replacing the identity
   connection).
1. Only paths which are connected by the identity transform are effectively considered to be part of the same
   space. All others are considered to be disjoint.

Note that in the absence of transforms, all entity paths are fully connected by the identity transform, and therefore
share the same space. However, as soon as you begin to log transforms, you can end up with additional spaces.

Consider the following scenario:

```python
rr.log("world/mapped_keypoints", rr.Points3D(…))
rr.log("world/robot/observed_features",rr.Points3D(…))
rr.log("world/robot", rr.Transforms3D(…))
```

There are 4 parent/child entity relationships represented in this hierarchy.

-   `(root)` -> `world`
-   `world` -> `world/mapped_keypoints`
-   `world` -> `world/robot`
-   `world/robot` -> `world/robot/observed_features`

The call: `rr.log("world/robot", rr.Transforms3D(…))` only applies to the relationship: `world` -> `world/robot` because the
logged transform (`world/robot`) describes the relationship between the entity and its _parent_ (`world`). All other
relationships are considered to be an identity transform.

This leaves us with two spaces. In one space, we have the entities `world`, and `world/mapped_keypoints`. In the other
space we have the entities `world/robot` and `world/robot/observed_features`.

Practically speaking, this means that the position values of the points from `world/mapped_keypoints` and the points
from `world/robot/observed_features` are not directly comparable. If you were to directly draw these points in a single
coordinate system the results would be meaningless. As noted above, Rerun can still display these entities in the same
view because it is able to automatically transform data between different spaces.

## Space transformations

In order to correctly display data from different spaces in the same view, Rerun uses the information from logged
transforms. Since most transforms are invertible, Rerun can usually transform data from a parent space to a child space
or vice versa. As long as there is a continuous chain of well-defined transforms, Rerun will apply the correct series
of transformations to the component data when building the scene.

Rerun transforms are currently limited to connections between _spatial_ views of 2D or 3D data. There are 3 types of
transforms that can be logged:

-   Affine 3D transforms, which can define any combination of translation, rotation, and scale relationship between two paths (see
    [`rr.Transform3D`](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Transform3D)).
-   Pinhole transforms define a 3D -> 2D camera projection (see
    [`rr.Pinhole`](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Pinhole)).

In the future, Rerun will be adding support for additional types of transforms.

-   [#349: Log 2D -> 2D transformations in the transform hierarchy](https://github.com/rerun-io/rerun/issues/349)

## Examples

Say you have a 3D world with two cameras with known extrinsics (pose) and intrinsics (pinhole model and resolution). You want to log some things in the shared 3D space, and also log each camera image and some detection in these images.

```py
# Log some data to the 3D world:
rr.log("world/points", rr.Points3D(…))

# Log first camera:
rr.log("world/camera/0", rr.Transform3D(translation=cam0_pose.pos, mat3x3=cam0_pose.rot))
rr.log("world/camera/0/image", rr.Pinhole(…))

# Log second camera:
rr.log("world/camera/1", rr.Transform3D(translation=cam1_pose.pos, mat3x3=cam1_pose.rot))
rr.log("world/camera/1/image", rr.Pinhole(…))

# Log some data to the image spaces of the first camera:
rr.log("world/camera/0/image", rr.Image(…))
rr.log("world/camera/0/image/detection", rr.Boxes2D(…))
```

Rerun will from this understand how the `world` space and the two image spaces (`world/camera/0/image` and `world/camera/1/image`) relate to each other, which allows you to explore their relationship in the Rerun Viewer. In the 3D view you will see the two cameras show up with their respective camera frustums (based on the intrinsics). If you hover your mouse in one of the image spaces, a corresponding ray will be shot through the 3D space.

Note that none of the names in the paths are special.

## View coordinates

You can use [`rr.ViewCoordinates`](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.ViewCoordinates) to set your preferred view coordinate systems, giving semantic meaning to the XYZ axes of the space.

For 3D spaces it can be used to log what the up-axis is in your coordinate system. This will help Rerun set a good default view of your 3D scene, as well as make the virtual eye interactions more natural. This can be done with `rr.log("/", rr.ViewCoordinates.RIGHT_HAND_Z_UP, static=True)`.
Note that in this example the archetype is logged at the root path, this will make it apply to all 3D views. Generally, a 3D view picks up view coordinates at or above its origin entity path.

You can also use this `log_view_coordinates` for pinhole entities, but it is encouraged that you instead use [`rr.log(…, rr.Pinhole(camera_xyz=…))`](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Pinhole) for this. The default coordinate system for pinhole entities is `RDF` (X=Right, Y=Down, Z=Forward).

WARNING: unlike in 3D views where `rr.ViewCoordinates` only impacts how the rendered scene is oriented, applying `rr.ViewCoordinates` to a pinhole-camera will actually influence the projection transform chain. Under the hood this value inserts a hidden transform that re-orients the axis of projection. Different world-content will be projected into your camera with different orientations depending on how you choose this value. See for instance the `open_photogrammetry_format` example.

For 2D spaces and other entities the view coordinates currently do nothing ([#1387](https://github.com/rerun-io/rerun/issues/1387)).
