---
title: Supported Message Formats
order: 400
---

Rerun provides automatic visualization for common message types in MCAP files through different processing layers.

## Protobuf Messages

The `protobuf` layer automatically decodes protobuf-encoded messages using schema reflection. Fields become queryable components, but no automatic visualizations are created.

## ROS2 Message Types

The `ros2msg` layer provides automatic visualization for common ROS2 message types:

- **`sensor_msgs`**: Images, point clouds, IMU data, camera info, joint states
- **`std_msgs`**: Basic data types and text messages

## ROS1 Message Types

ROS1 messages are not currently supported for semantic interpretation through any layer.
The `raw` and `schema` layers are able to preserve the original bytes and structure of the messages.

## Adding Support for New Types

To request support for additional message types:

- [File a GitHub issue](https://github.com/rerun-io/rerun/issues) requesting the specific message type
- Join the Rerun community on [Discord](https://discord.gg/PXtCgFBSmH) to discuss and collaborate with others
- Consider contributing a parser implementation
