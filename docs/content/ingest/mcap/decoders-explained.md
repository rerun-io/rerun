---
title: MCAP Decoders Explained
order: 300
description: How each MCAP decoder interprets and extracts data from the same file
---

MCAP processing in Rerun uses a decoder architecture where each decoder represents a different way to interpret and extract data from the same MCAP source.
By default, all decoders are enabled, including the `foxglove` lenses and the `urdf` decoder; when converting, you can select a subset of them.
The [`--decoder` option of `rerun mcap convert`](../../reference/cli.md#rerun-mcap-convert) lists every decoder name.

## Understanding decoders with an example

File-level decoders (`schema`, `stats`, `metadata`, `attachments`, `recording_info`) each run once over the file.
Message decoders are tried per channel in priority order (`ros2msg`, `ros2_reflection`, `protobuf`), and each channel is handled by the first enabled one that supports it.
The `raw` decoder handles the channels no other decoder supports; `rerun mcap convert --disable-raw-fallback` turns that off.
The `foxglove` lenses then turn decoded Foxglove Protobuf messages into Rerun archetypes.

Consider an MCAP file from a ROS2 robot containing sensor data on the topic `/robot/camera/image_raw` with ROS2 `sensor_msgs/msg/Image` messages:

- With only the `ros2msg` decoder: Creates an [Image](../../reference/types/archetypes/image.md) archetype for direct visualization in Rerun's viewer
- With only the `raw` decoder: Creates an [McapMessage](../../reference/types/archetypes/mcap_message.md) containing the original CDR-encoded message bytes
- With both decoders enabled: `ros2msg` handles the channel, and `raw` is only used for channels that `ros2msg` cannot decode

## Schema and statistics decoders

The `schema` decoder extracts structural information about the MCAP file's organization, creating metadata entities that describe channel definitions, topic names with their message types, and schema definitions.

The `stats` decoder computes file-level metrics and statistics, such as message counts per channel, temporal ranges, and file size, and stores them in `__mcap_properties`.

The `metadata` decoder copies the MCAP file's metadata records (if any) to `__mcap_metadata`, and the `attachments` decoder copies its attachment records (if any) to `__mcap_attachments` as static data.

## Message interpretation decoders

### Semantic interpretation

The `ros2msg` and `foxglove` decoders provide semantic interpretation and visualization of standard ROS 2 and Foxglove message types, creating meaningful Rerun visualization archetypes from data. Unlike the `protobuf` decoder, these decoders understand the semantics of the messages and create appropriate visualizations: images become [Image](../../reference/types/archetypes/image.md), point clouds become [Points3D](../../reference/types/archetypes/points3d.md), IMU messages become [Scalars](../../reference/types/archetypes/scalars.md) plotted over time, and so on.

See [Message Formats](message-formats.md) for the complete list of supported message types.

### Protobuf decoding

The `protobuf` decoder automatically decodes protobuf-encoded messages using reflection, creating structured component data based on the protobuf schema. Message fields become Rerun components that you can query and analyze.

## The raw decoder

The `raw` decoder preserves the original message bytes without any interpretation, creating blob entities containing the unprocessed message data. Each message appears as a binary blob that can be accessed programmatically for custom analysis tools.

## Recording info

The `recording_info` decoder extracts metadata about the recording session and capture context, such as recording timestamps, source system details, and capture software versions, and stores it in `__mcap_properties`.

## The URDF option

The `urdf` option uses Rerun's built-in URDF loader if there is a ROS 2 string topic named `/robot_description`, logging the robot model as static 3D geometry. In this MCAP workflow, joint transforms are not loaded from the URDF itself; they are expected to come from TF topics in the MCAP (e.g. `/tf` or `/tf_static`).

For general information about how to load URDF files, see [here](../urdf.md).

## Selecting decoders

To select decoders when converting MCAP files, see the [MCAP CLI reference](cli-reference.md#decoder-selection).

## Accessing decoder data

Each decoder creates different types of components on entity paths (derived from MCAP channel topics) that can be accessed through Rerun's SDK:

- Data from the `ros2msg` decoder and supported Foxglove messages appears as native Rerun visualization archetypes (see [here](message-formats.md#overview) for an overview)
- Other data from the `protobuf` or `ros2_reflection` decoders appears as structured components that can be queried by field name or manually added to certain views ([example](message-formats.md#example-time-series-plot-for-custom-message-scalars))
- Data from the `raw` decoder appears as blob components containing the original message bytes
- Data from the `urdf` option appears as static 3D robot geometry loaded from the ROS 2 `/robot_description` topic
- The `schema` decoder's static summary of channels and schemas appears as dedicated metadata entities
- Metadata from the `stats` and `recording_info` decoders appears in `__mcap_properties`, MCAP metadata records in `__mcap_metadata`, and attachments in `__mcap_attachments`

For more information on querying data and working with archetypes, see the [Data Queries documentation](../../catalog/get-data-out.md).

Each of these decoders contributes their own [chunks](../../log/chunks-and-rrd/chunks.md) to the Rerun-native data.
Below is a table showing the mapping between MCAP data and Rerun components:

| MCAP Data        | Rerun component                 | Description                                                                   |
| ---------------- | ------------------------------- | ----------------------------------------------------------------------------- |
| Schema name      | `McapSchema:name`               | Message type name from schema definition                                      |
| Schema data      | `McapSchema:data`               | Raw schema definition (protobuf, ROS2 msg, etc.)                              |
| Schema encoding  | `McapSchema:encoding`           | Schema format type                                                            |
|                  |                                 |                                                                               |
| Channel topic    | `McapChannel:topic`             | Topic name from MCAP channel                                                  |
| Channel ID       | `McapChannel:id`                | Numeric channel identifier                                                    |
| Message encoding | `McapChannel:message_encoding`  | Encoding format (e.g., `protobuf`, `cdr`)                                     |
|                  |                                 |                                                                               |
| Statistics       | `McapStatistics`                | File-level metrics like message counts and time ranges                        |
| Raw message data | `McapMessage:data`              | Unprocessed message bytes stored as binary blobs, handled by the `raw` decoder. |
