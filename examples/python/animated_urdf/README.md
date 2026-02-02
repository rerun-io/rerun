<!--[metadata]
title = "URDF"
tags = ["3D", "Mesh", "URDF", "Animation"]
channel = "main"
include_in_manifest = true
thumbnail = "https://static.rerun.io/animated-urdf-thumbnail/02cd73ad1155db0a202392b1fd8f8036070ad888/480w.png"
thumbnail_dimensions = [480, 480]
-->

<picture>
  <img src="https://static.rerun.io/animated_urdf/ebdefa158ab6f26f9dc1cb1924fce4b846fe8db2/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/animated_urdf/ebdefa158ab6f26f9dc1cb1924fce4b846fe8db2/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/animated_urdf/ebdefa158ab6f26f9dc1cb1924fce4b846fe8db2/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/animated_urdf/ebdefa158ab6f26f9dc1cb1924fce4b846fe8db2/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/animated_urdf/ebdefa158ab6f26f9dc1cb1924fce4b846fe8db2/1200w.png">
</picture>

An example of how to load and animate a URDF given some changing joint angles.

## Logging and visualizing with Rerun

This example demonstrates how to:

1. Load and log a URDF file as a static resource
2. Parse the URDF structure using `UrdfTree`
3. Animate joints by logging dynamic transforms

The key steps are:

```python
import rerun as rr
import rerun.urdf import UrdfTree

# Log the URDF file once, as a static resource
rec.log_file_from_path(urdf_path, static=True)

# Load the URDF tree structure into memory
urdf_tree = UrdfTree.from_file_path(urdf_path)

# Animate joints by logging transforms
for joint in urdf_tree.joints():
    if joint.joint_type == "revolute":
        # compute_transform gives you a complete transform that is ready to log,
        # calculated from joint origin and the current angle and with the frame names set.
        transform = joint.compute_transform(angle)
        rec.log("transforms", transform)
```
