---
title: Supported Message Formats
order: 200
---

Rerun provides automatic visualization for common message types in MCAP files through different processing layers.

## ROS2 message types

The `ros2msg` layer provides automatic conversion of common ROS2 messages to Rerun archetypes that can be visualized, e.g.:

- **`sensor_msgs`**
- **`std_msgs`**
- **`geometry_msgs`**
- **`builtin_interfaces`**
- **`tf2_msgs`**

We are continually adding support for more standard ROS2 message types. For the complete list of currently supported messages, see the [ROS2 message parsers in our codebase](https://github.com/rerun-io/rerun/blob/main/crates/store/re_mcap/src/layers/ros2.rs).

### Timelines

In addition to the `message_log_time` and `message_publish_time` timestamps that are part of an MCAP message, some ROS message payloads can have an additional [`Header`]( https://docs.ros.org/en/noetic/api/std_msgs/html/msg/Header.html) that may also contain timestamp information. These timestamps are put onto specific `ros2_*` timelines.

Timestamps within Unix time range (1990-2100) create a `ros2_timestamp` timeline. Values outside this range create a `ros2_duration` timeline representing relative time from custom epochs.

### ROS 2 transforms and poses

[`tf2_msgs/TFMessage`](https://docs.ros2.org/foxy/api/tf2_msgs/msg/TFMessage.html) is converted to [`Transform3D`](https://rerun.io/docs/reference/types/archetypes/transform3d)s, with `parent_frame` and `child_frame` set according to the `frame_id` and `child_frame_id` of each `geometry_msgs/TransformStamped` contained in the `transforms` list.
The timestamps of the individual transforms are put onto the `ros2_*` timelines, allowing the viewer to resolve the spatial relationships between frames over time similar to a TF buffer in ROS.

> More general information about TF-style transforms in Rerun can be found [here](https://rerun.io/docs/concepts/transforms#named-transform-frames).

[`geometry_msgs/PoseStamped`](https://docs.ros2.org/foxy/api/geometry_msgs/msg/PoseStamped.html) is converted to [`InstancePoses3D`](https://rerun.io/docs/reference/types/archetypes/instance_poses3d) with a [`CoordinateFrame`](https://rerun.io/docs/reference/types/archetypes/coordinate_frame) on the same entity path.
You can visualize these poses in the viewer by selecting the entity and adding a `TransformAxes3D` visualizer in the selection panel:

<picture>
  <img src="https://static.rerun.io/pose_axis_visualizer/8c819d2771b3f6f7a6c981d305019eb0364dd60c/full.png" alt="TransformAxes3D visualizer">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/pose_axis_visualizer/8c819d2771b3f6f7a6c981d305019eb0364dd60c/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/pose_axis_visualizer/8c819d2771b3f6f7a6c981d305019eb0364dd60c/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/pose_axis_visualizer/8c819d2771b3f6f7a6c981d305019eb0364dd60c/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/pose_axis_visualizer/8c819d2771b3f6f7a6c981d305019eb0364dd60c/1200w.png">
</picture>

> **Note:** the visualization requires that the coordinate frame of the pose is known, i.e. is part of the transform hierarchy of your data.

`CoordinateFrame`s are also added to other message types that are supported by the `ros2msg` layer and have an [`std_msgs/Header`](https://docs.ros2.org/foxy/api/std_msgs/msg/Header.html).
For data that can be visualized in 3D views (e.g. point clouds), this means that the viewer takes the respective coordinate frame's transform into account and renders the data relative to it.

## ROS2 reflection

The `ros2_reflection` layer automatically decodes ROS2 messages using runtime reflection for message types that are not supported by the semantic `ros2msg` layer. Fields become queryable components in the dataframe view and selection panel, but no automatic visualizations are created.

## ROS1 message types

ROS1 messages are not currently supported for semantic interpretation through any layer.
The `raw` and `schema` layers are able to preserve the original bytes and structure of the messages.

## Protobuf messages

The `protobuf` layer automatically decodes protobuf-encoded messages using schema reflection. Fields become queryable components, but no automatic visualizations are created.

## Adding support for new types

To request support for additional message types:

- [File a GitHub issue](https://github.com/rerun-io/rerun/issues) requesting the specific message type
- Join the Rerun community on [Discord](https://discord.gg/PXtCgFBSmH) to discuss and provide feedback on message support priorities. Or if you're open for a conversation, [sign up here](https://rerun.io/feedback)
