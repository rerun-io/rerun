---
title: Supported Message Formats
order: 200
---

Rerun provides automatic visualization for common message types in MCAP files through different processing layers.

## ROS2 message types

The `ros2msg` layer provides automatic visualization for common ROS2 message types:

- **`sensor_msgs`**
- **`std_msgs`**
- **`geometry_msgs`**
- **`builtin_interfaces`**

We are continually adding support for more standard ROS2 message types. For the complete list of currently supported messages, see the [ROS2 message parsers in our codebase](https://github.com/rerun-io/rerun/blob/main/crates/store/re_mcap/src/layers/ros2.rs).

### Timelines

In addition to the `message_log_time` and `message_publish_time` timestamps that are part of an MCAP message, some ROS message payloads can have an additional [`Header`]( https://docs.ros.org/en/noetic/api/std_msgs/html/msg/Header.html) that may also contain timestamp information. These timestamps are put onto specific `ros2_*` timelines.

Timestamps within Unix time range (1990-2100) create a `ros2_timestamp` timeline. Values outside this range create a `ros2_duration` timeline representing relative time from custom epochs.

### Limitations

The following are known limitations and link to the corresponding GitHub issues.

<!-- TODO(#11174) -->
- [Cannot express transforms defined via `tf` messages](https://github.com/rerun-io/rerun/issues/11174)

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
