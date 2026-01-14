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

### ROS 2 transforms (TF)

[`tf2_msgs/TFMessage`](https://docs.ros2.org/foxy/api/tf2_msgs/msg/TFMessage.html) is converted to [`Transform3D`](../../reference/types/archetypes/transform3d.md), with `parent_frame` and `child_frame` set according to the `frame_id` and `child_frame_id` of each `geometry_msgs/TransformStamped` contained in the message's `transforms` list.
The timestamps of the individual transforms are put onto the `ros2_*` timelines, allowing the viewer to resolve the spatial relationships between frames over time similar to a TF buffer in ROS.

> You can read more about how Rerun handles transforms and "TF-style" frame names [here](https://rerun.io/docs/concepts/transforms#named-transform-frames).

To see the transforms in the viewer, you can select the entity corresponding to the topic and add a visualizer for `TransformAxes3D` as shown in the video here.
If you have transforms that correspond to joints in a robot model, you can also read more about how to load `URDF` models into a recording [here](https://rerun.io/docs/howto/urdf#load-urdf-into-an-existing-recording).

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/83f26961023d5f554175ebc48d1292e218db1212_add_axes_visualizer.mp4" type="video/mp4" />
</video>

### ROS 2 poses and frame IDs

[`geometry_msgs/PoseStamped`](https://docs.ros2.org/foxy/api/geometry_msgs/msg/PoseStamped.html) is converted to [`InstancePoses3D`](../../reference/types/archetypes/instance_poses3d.md) with a [`CoordinateFrame`](../../reference/types/archetypes/coordinate_frame.md) on the same entity path.
Just like `Transform3D`, you can visualize these poses in the viewer by selecting the entity and adding a `TransformAxes3D` visualizer in the selection panel.
Note that the visualization requires the parent coordinate frame of the pose to be known, i.e. part of the transform hierarchy of your data.

[`CoordinateFrame`](../../reference/types/archetypes/coordinate_frame.md)s are also used for other message types that are supported by the `ros2msg` layer, if they have an [`std_msgs/Header`](https://docs.ros2.org/foxy/api/std_msgs/msg/Header.html) with a `frame_id`.
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
