---
title: Supported Message Formats
order: 200
---

Rerun provides automatic visualization for common message types in MCAP files:

* ROS 2 messages
* Foxglove schemas (Protobuf)

## Overview

This table shows an overview of the ROS 2 and Foxglove message schemas that are automatically converted to Rerun archetypes.

We are continually adding support for more standard message types.

| Modality | ROS 2 | Foxglove Protobuf | Rerun Archetypes |
| --- | --- | --- | --- |
| Raw image | `sensor_msgs/Image` | `RawImage` | [Image](../../../reference/types/archetypes/image.md), [DepthImage](../../../reference/types/archetypes/depth_image.md) |
| Encoded image | `sensor_msgs/CompressedImage` | `CompressedImage` | [EncodedImage](../../../reference/types/archetypes/encoded_image.md), [EncodedDepthImage](../../../reference/types/archetypes/encoded_depth_image.md) |
| Video | `sensor_msgs/CompressedImage` (h264) | `CompressedVideo` | [VideoStream](../../../reference/types/archetypes/video_stream.md) |
| Camera calibration | `sensor_msgs/CameraInfo` | `CameraCalibration` | [Pinhole](../../../reference/types/archetypes/pinhole.md) |
| Point cloud | `sensor_msgs/PointCloud2` | `PointCloud` | [Points3D](../../../reference/types/archetypes/points3d.md) |
| Geo points | `sensor_msgs/NavSatFix` | `LocationFix`, `LocationFixes`* | [GeoPoints](../../../reference/types/archetypes/geo_points.md) |
| Transforms | `tf2_msgs/TFMessage` | `FrameTransform`, `FrameTransforms` | [Transform3D](../../../reference/types/archetypes/transform3d.md) |
| Poses | `geometry_msgs/PoseStamped` | `PoseInFrame`, `PosesInFrame` | [InstancePoses3D](../../../reference/types/archetypes/instance_poses3d.md) |
| Coordinate frame | `.frame_id` field in `std_msgs/Header` | `.frame_id` field | [CoordinateFrame](../../../reference/types/archetypes/coordinate_frame.md)
| Magnetic field | `sensor_msgs/MagneticField` | - | [Arrows3D](../../../reference/types/archetypes/arrows3d.md) |
| Misc. scalar sensor data | `sensor_msgs/Imu`, `sensor_msgs/JointState`, `sensor_msgs/Temperature`, `sensor_msgs/FluidPressure`, `sensor_msgs/RelativeHumidity`, `sensor_msgs/Illuminance`, `sensor_msgs/Range`, `sensor_msgs/BatteryState` | - | [Scalars](../../../reference/types/archetypes/scalars.md) |
| Text | `std_msgs/String` | - | [TextDocument](../../../reference/types/archetypes/text_document.md) |
| Log messages | `rcl_interfaces/Log` | `Log` | [TextLog](../../../reference/types/archetypes/text_log.md) |

> *Support for `LocationFix` is coming soon.

### Timelines

The MCAP data loader adds [timelines](../../../concepts/logging-and-ingestion/timelines.md) based on the message timestamps.

In addition to the `message_log_time` and `message_publish_time` timestamps that are part of every MCAP message, we also add timelines with the application-specific timestamps from ROS and Foxglove schemas.

#### ROS

Most ROS message payloads have an additional [`Header`]( https://docs.ros.org/en/noetic/api/std_msgs/html/msg/Header.html) that may also contain timestamp information. These timestamps are put onto specific `ros2_*` timelines.

Timestamps within Unix time range (1990-2100) create a `ros2_timestamp` timeline. Values outside this range create a `ros2_duration` timeline representing relative time from custom epochs.

#### Foxglove

Data from schemas containing a `.timestamp` field is put onto a `timestamp` timeline.

### Transforms (TF)

Transform messages are converted to [`Transform3D`](../../../reference/types/archetypes/transform3d.md), with `parent_frame` and `child_frame` set according to the `frame_id` and `child_frame_id` of each `geometry_msgs/TransformStamped` contained in the message's `transforms` list.
The timestamps of the individual transforms are put onto the respective timelines, allowing the viewer to resolve the spatial relationships between frames over time similar to a TF buffer in ROS.

> You can read more about how Rerun handles transforms and "TF-style" frame names [here](https://rerun.io/docs/concepts/transforms#named-transform-frames).

To see the transforms in the viewer, you can select the entity corresponding to the topic and add a visualizer for `TransformAxes3D` as shown in the video here.
If you have transforms that correspond to joints in a robot model, you can also read more about how to load `URDF` models into a recording [here](https://rerun.io/docs/howto/urdf#load-urdf-into-an-existing-recording).

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/83f26961023d5f554175ebc48d1292e218db1212_add_axes_visualizer.mp4" type="video/mp4" />
</video>

### Poses and frame IDs

Pose messages are converted to [`InstancePoses3D`](../../../reference/types/archetypes/instance_poses3d.md) with a [`CoordinateFrame`](../../../reference/types/archetypes/coordinate_frame.md) on the same entity path.
Just like `Transform3D`, you can visualize these poses in the viewer by selecting the entity and adding a `TransformAxes3D` visualizer in the selection panel.
Note that the visualization requires the parent coordinate frame of the pose to be known, i.e. part of the transform hierarchy of your data.

[`CoordinateFrame`](../../../reference/types/archetypes/coordinate_frame.md)s are also used for other message types that are supported by the `ros2msg` layer, if they have an [`std_msgs/Header`](https://docs.ros2.org/foxy/api/std_msgs/msg/Header.html) with a `frame_id`.
For data that can be visualized in 3D views (e.g. point clouds), this means that the viewer takes the respective coordinate frame's transform into account and renders the data relative to it.

## ROS2 reflection

The `ros2_reflection` layer automatically decodes ROS2 messages using runtime reflection for message types that are not supported by the semantic `ros2msg` layer. Fields become queryable components in the dataframe view and selection panel, but no automatic visualizations are created.

## ROS1 message types

ROS1 messages are currently not supported for semantic interpretation through any layer.
The `raw` and `schema` layers are able to preserve the original bytes and structure of the messages.

## Protobuf messages

Not all Foxglove Protobuf messages are currently supported. Additionally, MCAP files allow for custom Protobuf definitions and are not restricted to the Foxglove Protobuf schemas.

The `protobuf` layer automatically decodes these unknown protobuf-encoded messages using schema reflection. Fields become queryable components (e.g. for training data curation), but no automatic visualizations are created.
Depending on the contents of your data, you can still manually add visualizers like time-series or dataframe views to your blueprint.

## Adding support for new types

To request support for additional message types:

- [File a GitHub issue](https://github.com/rerun-io/rerun/issues) requesting the specific message type
- Join the Rerun community on [Discord](https://discord.gg/PXtCgFBSmH) to discuss and provide feedback on message support priorities. Or if you're open for a conversation, [sign up here](https://rerun.io/feedback)
