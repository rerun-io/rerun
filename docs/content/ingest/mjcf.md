---
title: Loading MJCF models
order: 550
description: Load MuJoCo robot models defined in MJCF files and visualize them in the 3D view
---

Rerun features a built-in [importer](importers/overview.md) for [MJCF](https://mujoco.readthedocs.io/en/stable/XMLreference.html) files, the model format of the [MuJoCo](https://mujoco.org/) physics engine.

## Overview

Load an MJCF file with the logging API, drag it into the Viewer, or open it from the command line (`rerun robot.xml`).
Both `.mjcf` files and `.xml` files whose root element is `<mujoco>` are recognized.

The importer takes care of:
* resolving `<include>`s and `<default>` classes
* loading meshes and primitive shapes as Rerun entities
* logging one named frame per body, with the static transforms of the model's reference configuration on `/tf_static`

Once that is done, the joints can be updated by sending [`Transform3D`](../reference/types/archetypes/transform3d.md)s with the `parent_frame` and `child_frame` of each joint.
The [`rr.mjcf`](#mjcf-utilities-python) Python module computes these transforms from joint values.

```python
import rerun as rr

rr.init("rerun_example_load_mjcf", spawn=True)

# `log_file_from_path` automatically uses the built-in MJCF importer
# if the XML file starts with a `<mujoco>` root element.
rr.log_file_from_path("robot.xml", static=True)
```

## Frames

Every body has a frame named after it.
The root frame is `world`, and an unnamed body is called `body_<id>`, where `<id>` is its MuJoCo body id.

A joint moves its body relative to the body's parent.
Unlike a [URDF](urdf.md) link, an MJCF body can have several joints, which MuJoCo applies in order.
So that every joint can be animated on its own, such a body gets an intermediate frame after each joint but its last, named `{body}::{joint}`.
A body `forearm` with the joints `yaw` and `pitch` below a body `upper_arm` gives this chain:

```
upper_arm → forearm::yaw → forearm
```

The `yaw` joint moves `forearm::yaw` relative to `upper_arm`, and the `pitch` joint moves `forearm` relative to `forearm::yaw`.

## Geometry

The importer logs the geometry under an entity named after the model, which is its `model` attribute or else its file name:

- `<model_name>/visual_geometries/<body_name>/<geom_name>` for the visual geometries.
- `<model_name>/collision_geometries/<geom_type>/<body_name>/<geom_name>` for the collision geometries.

MJCF does not mark geoms as visual or collision, so the importer follows MuJoCo's convention: geoms in groups 3 and above, which MuJoCo's own viewer hides by default, are collision geometries.
Collision geometries are grouped under a separate entity path root that can be toggled in the blueprint.

An unnamed geom is called `geom_<n>`, where `<n>` counts the geoms of its body that can be visualized.

Supported geom types are `plane`, `sphere`, `capsule`, `ellipsoid`, `cylinder`, `box`, and `mesh`, including the `fromto` attribute.
Geoms are colored by their `rgba` attribute or by the `rgba` of their material.

Mesh files are resolved like in MuJoCo, relative to the `meshdir` of the `<compiler>` element or else to the folder of the model file.
They can be in any format that [`Asset3D`](../reference/types/archetypes/asset3d.md) supports, including `.stl` and `.obj`.
The `scale` attribute of a `<mesh>` is applied, and meshes declared inline with `vertex` and `face` are supported too.
A mesh that cannot be loaded is skipped with a warning, and the rest of the model is still imported.

### Limitations

- Heightfield (`hfield`) and `sdf` geoms are skipped with a warning.
- Infinite planes (a plane with a zero size) are skipped.
- Inline meshes without `face` data, MuJoCo's `.msh` files, and textures are not supported.
- The `<replicate>`, `<composite>`, and `<flexcomp>` elements are not supported.
- A model opened in the web Viewer or from an HTTP URL cannot read other files, so its `<include>`s and mesh files are not loaded.
- Opening a folder imports every MJCF file in it on its own, so a model that is also included by another file is loaded twice.
  Open the main file instead.

## MJCF utilities (Python)

The [`rr.mjcf`](https://ref.rerun.io/docs/python/stable/mjcf/) Python module gives access to the bodies and joints of a model, and computes the transforms of its joints from joint values.
It mirrors the [`rr.urdf`](urdf.md#urdf-utilities-python) module.

### MjcfTree

Load an MJCF file and access its structure:

```python
mjcf_tree = rr.mjcf.MjcfTree.from_file_path("robot.xml", entity_path_prefix=None)

# Access properties
model_name = mjcf_tree.name
world = mjcf_tree.root_body()
bodies = mjcf_tree.bodies()
joints = mjcf_tree.joints()

# Lookup by name
mjcf_tree.get_joint_by_name("shoulder")
mjcf_tree.get_body_by_name("forearm")

# Get the entity paths of the visual or collision geometries of a body,
# for example to override their properties in a blueprint.
visual_paths = mjcf_tree.get_visual_geometry_paths("gripper")
collision_paths = mjcf_tree.get_collision_geometry_paths("gripper")
```

Use `log_to_recording()` to log the model (geometry and static transforms), as an alternative to `log_file_from_path`:

```python
mjcf_tree.log_to_recording()
```

#### Frame prefix

When loading the same model multiple times (e.g. a dual-arm setup), use `frame_prefix` to give each instance unique frame IDs and `entity_path_prefix` to separate their geometry in the entity tree:

```python
left = rr.mjcf.MjcfTree.from_file_path("robot.xml", entity_path_prefix="left", frame_prefix="left/")
right = rr.mjcf.MjcfTree.from_file_path("robot.xml", entity_path_prefix="right", frame_prefix="right/")

left.log_to_recording()
right.log_to_recording()
```

Transforms computed via `joint.compute_transform()` automatically use the prefixed frame IDs (e.g. `"left/world"`, `"right/forearm"`).

The static transforms are logged to `/tf_static`, regardless of `entity_path_prefix`.
Pass `static_transform_entity_path` to log them to another entity.

### MjcfBody

Each body exposes its `name`, the name of its `parent` body (`None` for the world body), and its `joints()` in the order MuJoCo applies them.
Its `translation` and `quaternion_xyzw` are its pose relative to the parent body when all joints are at their reference value.
The quaternion is in `xyzw` order, like Rerun's, while MJCF and MuJoCo use `wxyz`.

### MjcfJoint

Each joint exposes properties from the MJCF file:

* `name` (an unnamed joint is called `joint_<id>`, where `<id>` is its MuJoCo joint id)
* `joint_type` (`hinge`, `slide`, `ball`, or `free`)
* `body`, `parent_body`
* `axis`, `anchor` (the joint's `pos`), both in the body's frame
* `reference` (the joint's `ref`)
* `range` (`None` if the joint is not limited)

Angles are always in radians, also if the model uses `<compiler angle="degree"/>`.

Use `compute_transform()` to get a [`Transform3D`](../reference/types/archetypes/transform3d.md) with the correct `parent_frame` and `child_frame` already set:

```python
# For hinge joints: pass the angle in radians
# For slide joints: pass the distance in meters
transform = joint.compute_transform(value)
rec.log("transforms", transform)
```

The value follows the convention of MuJoCo's `qpos`, so the values of a simulation can be passed as they are:

```python
for joint in mjcf_tree.joints():
    if joint.joint_type in ("hinge", "slide"):
        value = data.joint(joint.name).qpos[0]  # `data` is a `mujoco.MjData`
        rec.log("transforms", joint.compute_transform(value, clamp=False))
```

By default, values outside the joint's `range` are clamped, with a warning.
Pass `clamp=False` to use the value as is.

To send many joint values at once, use `compute_transform_columns()` with [`send_columns`](../log/send-data/send-columns.md):

```python
angles = [0.0, 0.1, 0.2, 0.3]
rec.send_columns(
    "transforms",
    indexes=[rr.TimeColumn("step", sequence=range(len(angles)))],
    columns=joint.compute_transform_columns(angles),
)
```

#### Ball and free joints

Only hinge and slide joints have a single value, so `compute_transform()` raises `NotImplementedError` for ball and free joints.
`MjcfTree.compute_joint_transform_batches()` does the same if one of the joint names refers to such a joint, so leave these joints out.

To move a body with a free joint, such as the floating base of a legged robot, log the transform from its parent to the body yourself.
The `qpos` of a free joint holds the position, followed by the orientation as a quaternion in `wxyz` order:

```python
x, y, z, qw, qx, qy, qz = data.joint(joint.name).qpos
rec.log(
    "transforms",
    rr.Transform3D(
        translation=[x, y, z],
        quaternion=rr.Quaternion(xyzw=[qx, qy, qz, qw]),
        parent_frame=joint.parent_body,
        child_frame=joint.body,
    ),
)
```

If the tree has a `frame_prefix`, prepend it to both frame IDs.

### Streaming the model as chunks

`MjcfTree.stream()` returns the chunks of the model as an experimental `rerun.chunk.LazyChunkStream`, for use with the [Chunk Processing API](chunk-processing-api.md).
Pass `include_joint_transforms=False` to leave out the static joint transforms.

## References

* [🐍 Python `log_file_from_path`](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.log_file_from_path)
* [🦀 Rust `log_file_from_path`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.log_file_from_path)
* [🌊 C++ `log_file_from_path`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#a20798d7ea74cce5c8174e5cacd0a2c47)
* [Loading URDF models](urdf.md)
