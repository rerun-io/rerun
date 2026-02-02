---
title: MCAP Layers Explained
order: 300
---

MCAP processing in Rerun uses a layered architecture where each layer represents a different way to interpret and extract data from the same MCAP source.
By default, when opening a file Rerun analyzes an MCAP file to determine which layers are active to provide the most comprehensive view of your data, while avoiding duplication.
You can specify which layers to use during conversion, allowing you to extract exactly the information you need for your analysis.

## Understanding layers with an example

When multiple layers are enabled, they each process the same messages independently, creating different component types on identical entity paths. This can result in data duplication—for instance, enabling both `raw` and `protobuf` layers stores the same message as both structured field data and raw binary blobs.

Consider an MCAP file from a ROS2 robot containing sensor data on the topic `/robot/camera/image_raw` with ROS2 `sensor_msgs/msg/Image` messages:

- With only the `ros2msg` layer: Creates an [Image](../../../reference/types/archetypes/image.md) archetype for direct visualization in Rerun's viewer
- With only the `raw` layer: Creates an [McapMessage](../../../reference/types/archetypes/mcap_message.md) containing the original CDR-encoded message bytes
- With both layers enabled: All representations coexist on the same entity path `/robot/camera/image_raw`

## Schema and statistics layers

The `schema` layer extracts structural information about the MCAP file's organization, creating metadata entities that describe channel definitions, topic names with their message types, and schema definitions. This layer is particularly useful for understanding unfamiliar MCAP files or getting an overview of available topics and channels before deeper processing.

The `stats` layer computes file-level metrics and statistics, creating entities with message counts per channel, temporal ranges, file size information, and data rate analysis. This gives you insight into the scale and characteristics of your dataset for quality assessment and planning storage requirements.

## Message interpretation layers

### ROS2 semantic interpretation

The `ros2msg` layer provides semantic interpretation and visualization of standard ROS2 message types, creating meaningful Rerun visualization components from data. Unlike the `protobuf` layer, this layer understands the semantics of ROS2 messages and creates appropriate visualizations: images become [Image](../../../reference/types/archetypes/image.md), point clouds become [Points3D](../../../reference/types/archetypes/points3d.md), IMU messages become [SeriesLines](../../../reference/types/archetypes/series_lines.md) with the data plotted over time, and so on.

This layer supports standard ROS2 packages including `sensor_msgs`, `geometry_msgs`, `std_msgs`, and `builtin_interfaces`. This layer provides visualization of sensor data like cameras and LiDAR with minimal setup required.

See [Message Formats](message-formats.md) for the complete list of supported message types.

### Protobuf decoding

The `protobuf` layer automatically decodes protobuf-encoded messages using reflection, creating structured component data based on the protobuf schema. Message fields become Rerun components that you can query and analyze.

However, this layer provides structured access without semantic visualization meaning. While the data becomes queryable, it won't automatically appear as meaningful visualizations like images or point clouds, it gives you the data structure, not the visual interpretation.

## The raw layer

The `raw` layer preserves the original message bytes without any interpretation, creating blob entities containing the unprocessed message data. Each message appears as a binary blob that can be accessed programmatically for custom analysis tools.

## Recording info

The `recording_info` layer extracts metadata about the recording session and capture context, creating metadata entities with information about recording timestamps, source system details, and capture software versions.

## Layer selection and performance

### Selecting layers

By default, Rerun processes MCAP files with all layers active. You can control which layers are used when [converting MCAP files via the CLI](cli-reference.md) using the `-l` flag:

```bash
# Use only specific layers
rerun mcap convert input.mcap -l protobuf -l stats -o output.rrd

# Use multiple layers for different perspectives
rerun mcap convert input.mcap -l ros2msg -l raw -l recording_info -o output.rrd
```

## Accessing layer data

Each layer creates different types of components on entity paths (derived from MCAP channel topics) that can be accessed through Rerun's SDK:

- Data from the `protobuf` layer appears as structured components that can be queried by field name
- Data from the `ros2msg` layer appears as native Rerun visualization components ([Image](../../../reference/types/archetypes/image.md), [Points3D](../../../reference/types/archetypes/points3d.md), etc.)
- Data from the `raw` layer appears as blob components containing the original message bytes
- Metadata from `schema`, `stats`, and `recording_info` layers appears as dedicated metadata entities

For more information on querying data and working with archetypes, see the [Data Queries documentation](../../../howto/query-and-transform/get-data-out.md).

Each of these layers contributes their own [chunks](../chunks.md) to the Rerun-native data.
Below is a table showing the mapping between MCAP data and Rerun components:

| MCAP Data        | Rerun component                 | Description                                                                   |
| ---------------- | ------------------------------- | ----------------------------------------------------------------------------- |
| Schema name      | `mcap.Schema:name`              | Message type name from schema definition                                      |
| Schema data      | `mcap.Schema:data`              | Raw schema definition (protobuf, ROS2 msg, etc.)                              |
| Schema encoding  | `mcap.Schema:encoding`          | Schema format type                                                            |
|                  |                                 |                                                                               |
| Channel topic    | `mcap.Channel:topic`            | Topic name from MCAP channel                                                  |
| Channel ID       | `mcap.Channel:id`               | Numeric channel identifier                                                    |
| Message encoding | `mcap.Channel:message_encoding` | Encoding format (e.g., `protobuf`, `cdr`)                                     |
|                  |                                 |                                                                               |
| Statistics       | `mcap.Statistics`               | File-level metrics like message counts and time ranges                        |
| Raw message data | `mcap.Message:data`             | Unprocessed message bytes stored as binary blobs, handled by the `raw` layer. |
