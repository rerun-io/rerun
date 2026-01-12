---
title: Loading URDF models
order: 900
---

Rerun features a built-in [data-loader](https://rerun.io/docs/reference/data-loaders/overview) for [URDF](https://en.wikipedia.org/wiki/URDF) files.

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/urdf-viewer/ebdefa158ab6f26f9dc1cb1924fce4b846fe8db2/full.png" alt="A robot model loaded from an URDF file visualized in Rerun.">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/urdf-viewer/ebdefa158ab6f26f9dc1cb1924fce4b846fe8db2/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/urdf-viewer/ebdefa158ab6f26f9dc1cb1924fce4b846fe8db2/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/urdf-viewer/ebdefa158ab6f26f9dc1cb1924fce4b846fe8db2/1024w.png">
</picture>

## Overview

Using a `URDF` in Rerun only requires you to load the file with the logging API.
This will automatically invoke the data-loader, which will take care of:
* resolving paths to meshes
* loading meshes and shapes as Rerun entities
* loading the joint transforms and associated frame IDs of links

Once that is done, the joints can be updated by sending [`Transform3D`](../../reference/types/archetypes/transform3d.md)s, where you have to set the `parent_frame` and `child_frame` fields explicitly to each joint's specific frame IDs.

> ⚠️ Note: previous versions (< 0.28) required you to send transforms with _implicit_ frame IDs, i.e. having to send each joint transform on a specific entity path.
> This was dropped in favor of _named_ frame IDs, which is more in line with ROS and allows you to send all transform updates on one entity (e.g. a `transforms` entity like in the example below).

## Example

Here is an example that demonstrates how to load and update a `URDF` with the Python SDK:

snippet: howto/load_urdf

For a full animation example, see the [Python animated URDF example](https://github.com/rerun-io/rerun/tree/main/examples/python/animated_urdf). There's also a [Rust example](https://github.com/rerun-io/rerun/tree/main/examples/rust/animated_urdf).

## URDF utilities (Python)

Rerun provides the [`rr.urdf`](https://github.com/rerun-io/rerun/tree/main/rerun_py/rerun_sdk/rerun/urdf.py) Python module that can facilitate the handling of URDF models in your code.
It can be used as an alternative to other 3rd-party packages like [yourdfpy](https://yourdfpy.readthedocs.io/en/latest/index.html) or [pytransforms3d](https://dfki-ric.github.io/pytransform3d/index.html).
As shown below, you can use it e.g. to access individual joints of the URDF model and to compute their respective transforms based on joint states (e.g. angles for revolute joints).
These transforms can be directly sent to Rerun.

### UrdfTree

Load a URDF file and access its structure:

```python
urdf_tree = rr.urdf.UrdfTree.from_file_path("robot.urdf")

# Access properties
robot_name = urdf_tree.name
root_link = urdf_tree.root_link()
joints = urdf_tree.joints()

# Lookup by name
urdf_tree.get_joint_by_name("shoulder")
urdf_tree.get_link_by_name("base_link")
urdf_tree.get_link_path_by_name("end_effector")  # Entity path for logging additional data e.g. images
```

### UrdfJoint

Each joint exposes properties from the URDF file:

* `name`
* `joint_type` (e.g. `revolute`, `continuous`, `prismatic`, `fixed`)
* `parent_link`, `child_link`
* `axis`, `origin_xyz`, `origin_rpy`
* `limit_lower`, `limit_upper`, `limit_effort`, `limit_velocity`

Use `compute_transform()` to get a [`Transform3D`](../../reference/types/archetypes/transform3d.md) with the correct `parent_frame` and `child_frame` already set:

```python
# For revolute/continuous joints: pass angle in radians
# For prismatic joints: pass distance in meters
transform = joint.compute_transform(angle)
rec.log("transforms", transform)
```

## Load URDF into an existing recording

If you already have a recording with transforms loaded in Rerun and want to add an URDF to it, you can do so via drag-and-drop or the menu ("Import into current recording").

In this video, we load an ROS 2 `.mcap` file with TF messages that automatically get translated into Rerun [`Transform3D`](../../reference/types/archetypes/transform3d.md).
As indicated by the errors displayed in the viewer, there are some connections missing in the transform tree of this example MCAP.
In our case, these missing transforms are static links that are stored in URDF models separate from the MCAP file.

To add them, we can simply drag the corresponding URDF files into the viewer where we have loaded the MCAP:

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/69e08d9dcaed77f5f93190ffb9ccf75376c7d1c4_urdf_drag_and_drop.mp4" type="video/mp4" />
</video>

## References

* [🐍 Python `log_file_from_path`](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.log_file_from_path)
* [🦀 Rust `log_file_from_path`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.log_file_from_path)
* [🌊 C++ `log_file_from_path`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#a20798d7ea74cce5c8174e5cacd0a2c47)
