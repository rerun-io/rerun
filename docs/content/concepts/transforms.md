---
title: Transforms & Transform Frames
order: 300
---

## Transform hierarchies with entity paths

Rerun uses transforms to define spatial relationships between entities. The [`Transform3D`](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Transform3D) archetype allows you to specify how one coordinate system relates to another through translation, rotation, and scaling.

The simplest way to use transforms is through entity path hierarchies, where each transform describes the relationship between an entity and its parent path:

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
- The `sun/planet` transform places the planet 6 units away from the sun
- The `sun/planet/moon` transform places the moon 3 units away from the planet

This creates a transform hierarchy where transforms propagate down the entity tree. The moon's final position in the sun's coordinate system is 9 units away (6 + 3), because the transforms are applied sequentially.

## Explicit transform frames

⚠️ **Experimental feature**: Transform frames are still in early development and the API may change.

While entity path hierarchies work well for many cases, sometimes you need more flexibility in organizing your transforms. Explicit transform frames allow you to decouple the spatial relationships from the entity hierarchy by using named coordinate frames.

### Using named frames

With explicit frames, you specify `child_frame` and `parent_frame` parameters to define which coordinate systems a transform connects:

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

Key differences with explicit frames:
- **Decoupled organization**: Entities can be logged at any path (`/sun`, `/planet`, `/moon`)
- **Named relationships**: Transforms specify explicit frame names rather than using entity hierarchy
- **Frame assignment**: [`rr.CoordinateFrame`](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.CoordinateFrame) tells entities which frame they belong to
- **Flexible hierarchy**: The spatial hierarchy is independent of the entity path structure

### Transform types

Rerun supports several types of transforms:

- **Affine 3D transforms** ([`rr.Transform3D`](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Transform3D)): Define translation, rotation, and scale relationships between coordinate systems
- **Pinhole camera projections** ([`rr.Pinhole`](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Pinhole)): Define 3D → 2D camera projections with explicit frame support

## Entity-path derived frames: Bridging both approaches

Under the hood, Rerun's entity path hierarchies actually use the same transform frame system as explicit frames. When you don't specify `child_frame` and `parent_frame`, Rerun automatically creates implicit frames derived from entity paths.

### How implicit frames work

Each entity path automatically gets an associated transform frame with the prefix `tf#`:
- Entity `/world/robot` gets frame `tf#/world/robot`
- Entity `/world/robot/arm` gets frame `tf#/world/robot/arm`
- These implicit frames are connected by identity transforms following the path hierarchy

### Side-by-side comparison

Here are equivalent setups using both approaches:

**Traditional entity-path approach:**
```python
# Entities follow the path hierarchy
rr.log("world/robot", rr.Transform3D(translation=[1, 0, 0]))
rr.log("world/robot/arm", rr.Transform3D(translation=[0, 1, 0]))
rr.log("world/robot/arm/gripper", rr.Points3D([0, 0, 0]))
```

**Equivalent explicit frame approach:**
```python
# Entities can be organized independently of spatial relationships
rr.log("robot", rr.CoordinateFrame("robot_frame"))
rr.log("arm", rr.CoordinateFrame("arm_frame"))  
rr.log("gripper_points", rr.Points3D([0, 0, 0]), rr.CoordinateFrame("gripper_frame"))

# Spatial relationships defined separately
rr.log("robot_transform", rr.Transform3D(
    translation=[1, 0, 0],
    child_frame="robot_frame", 
    parent_frame="tf#/world"  # Connect to implicit world frame
))
rr.log("arm_transform", rr.Transform3D(
    translation=[0, 1, 0],
    child_frame="arm_frame",
    parent_frame="robot_frame"
))
rr.log("gripper_transform", rr.Transform3D(
    child_frame="gripper_frame",
    parent_frame="arm_frame"
))
```

Both approaches create the same spatial relationships but offer different trade-offs:
- **Entity paths**: Simple and intuitive, couples entity organization with spatial hierarchy
- **Explicit frames**: More flexible, allows complex spatial relationships independent of entity structure

### Camera example with both approaches

A practical example with cameras and 3D content:

```python
# Log 3D world content
rr.log("world/points", rr.Points3D(world_points))

# Traditional approach - cameras in entity hierarchy
rr.log("world/camera/0", rr.Transform3D(translation=cam0_pose.pos, mat3x3=cam0_pose.rot))
rr.log("world/camera/0/image", rr.Pinhole(image_from_camera=cam0_intrinsics))
rr.log("world/camera/0/image", rr.Image(cam0_image))
rr.log("world/camera/0/image/detections", rr.Boxes2D(cam0_detections))

# Explicit frame approach - same spatial result, different organization
rr.log("cam0", rr.CoordinateFrame("cam0_frame"))
rr.log("cam0_image_view", rr.CoordinateFrame("cam0_image_frame"))
rr.log("cam0_transform", rr.Transform3D(
    translation=cam0_pose.pos, mat3x3=cam0_pose.rot,
    child_frame="cam0_frame", parent_frame="tf#/world"
))
rr.log("cam0_projection", rr.Pinhole(
    image_from_camera=cam0_intrinsics,
    child_frame="cam0_image_frame", parent_frame="cam0_frame"
))
rr.log("cam0_image_view", rr.Image(cam0_image))
rr.log("detection_boxes", rr.Boxes2D(cam0_detections), rr.CoordinateFrame("cam0_image_frame"))
```

Rerun automatically handles the transform chain resolution, so both approaches result in the same spatial relationships and viewer behavior.

## View coordinates

You can use [`rr.ViewCoordinates`](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.ViewCoordinates) to define the semantic meaning of coordinate axes, helping Rerun orient 3D views naturally and making camera interactions more intuitive.

### 3D view coordinates

For 3D spaces, view coordinates specify the up-axis and handedness:

```python
# Entity-path approach - applies to all 3D views
rr.log("/", rr.ViewCoordinates.RIGHT_HAND_Z_UP, static=True)

# Explicit frame approach - can be applied per frame
rr.log("world_coords", rr.ViewCoordinates.RIGHT_HAND_Z_UP, rr.CoordinateFrame("world_frame"))
rr.log("robot_coords", rr.ViewCoordinates.RIGHT_HAND_Y_UP, rr.CoordinateFrame("robot_frame"))
```

Common coordinate systems:
- `RIGHT_HAND_Z_UP`: X=Right, Y=Forward, Z=Up (common in robotics)
- `RIGHT_HAND_Y_UP`: X=Right, Y=Up, Z=Back (common in graphics)
- `LEFT_HAND_Y_UP`: X=Right, Y=Up, Z=Forward (common in game engines)

### Camera coordinate systems

For cameras, you can specify view coordinates using the `camera_xyz` parameter in [`rr.Pinhole`](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Pinhole):

```python
# Traditional approach
rr.log("world/camera", rr.Pinhole(
    image_from_camera=intrinsics,
    camera_xyz=rr.ViewCoordinates.RDF  # X=Right, Y=Down, Z=Forward (default)
))

# Explicit frame approach  
rr.log("camera_projection", rr.Pinhole(
    image_from_camera=intrinsics,
    camera_xyz=rr.ViewCoordinates.RDF,
    child_frame="camera_image_frame",
    parent_frame="camera_frame"
))
```

**Important**: Unlike 3D view coordinates which only affect visualization, camera view coordinates actually influence the projection math. Different camera orientations will project world content differently based on this setting.

For 2D spaces and other entities, view coordinates currently have no effect ([#1387](https://github.com/rerun-io/rerun/issues/1387)).

## Summary

Rerun's transform system offers flexibility in how you organize spatial relationships:

- **Start simple** with entity-path hierarchies for straightforward cases
- **Use explicit frames** when you need to decouple entity organization from spatial relationships  
- **Mix both approaches** as needed - they work seamlessly together through the underlying frame system
- **Leverage view coordinates** to ensure natural 3D navigation and correct camera projections

The choice between approaches depends on your use case: entity paths for simplicity, explicit frames for flexibility, or a combination for complex scenarios.
