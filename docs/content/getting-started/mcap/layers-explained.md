---
title: MCAP Layers Explained
order: 300
---

MCAP processing in Rerun uses a layer-based architecture where each layer represents a different way to interpret and extract data from the same MCAP file. You can specify which layers to use during conversion, allowing you to extract exactly the information you need for your analysis.

When multiple layers are enabled, they each filter and process only the messages they're designed to handle based on encoding and schema compatibility, creating different components and archetypes on the same entity paths (derived from MCAP channel topics). This can result in data duplication when layers have overlapping interests—for example, enabling both `raw` and `protobuf` layers will store protobuf messages in two forms: once as structured field data and once as raw binary blobs.

Each of these layers contributes their own [chunks](../../concepts/chunks.md) to the Rerun-native data. The table below shows how MCAP data is mapped to Rerun's chunk structure:

| MCAP Data          | Rerun Storage                                   | Description                                                                                                                                                      |
| ------------------ | ----------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Raw message data   | `mcap.Message:data`                             | Unprocessed message bytes stored as binary blobs                                                                                                                 |
| Channel topic      | `mcap.Channel:topic`                            | Topic name from MCAP channel                                                                                                                                     |
| Schema name        | `mcap.Schema:name`                              | Message type name from schema definition                                                                                                                         |
| Schema data        | `mcap.Schema:data`                              | Raw schema definition (protobuf, ROS2 msg, etc.)                                                                                                                 |
| Channel ID         | `mcap.Channel:id`                               | Numeric channel identifier                                                                                                                                       |
| Message encoding   | `mcap.Channel:message_encoding`                 | Encoding format (e.g., `protobuf`, `cdr`)                                                                                                                        |
| Channel metadata   | `mcap.Channel:metadata`                         | Key-value pairs from channel metadata                                                                                                                            |
| Schema encoding    | `mcap.Schema:encoding`                          | Schema format type                                                                                                                                               |
| Message timestamps | [Timeline](../../concepts/timelines.md) columns | `log_time` and `publish_time` timelines                                                                                                                          |
| Statistics         | `mcap.Statistics`                               | File-level metrics like message counts and time ranges                                                                                                           |
| Protobuf fields    | Dynamic component columns                       | Decoded protobuf message fields as typed components                                                                                                              |
| ROS2 messages      | Visualization components                        | Semantic conversion to Rerun's native types ([Points3D](../../reference/types/archetypes/points3d.md), [Image](../../reference/types/archetypes/image.md), etc.) |

## Schema and statistics layers

The `schema` layer extracts structural information about the MCAP file's organization, creating metadata entities that describe channel definitions, topic names with their message types, and schema definitions. This layer is particularly useful for understanding unfamiliar MCAP files or getting an overview of available topics and channels before deeper processing.

The `stats` layer computes file-level metrics and statistics, creating entities with message counts per channel, temporal ranges, file size information, and data rate analysis. This gives you insight into the scale and characteristics of your dataset for quality assessment and planning storage requirements.

## Message interpretation layers

### ROS2 semantic interpretation

The `ros2msg` layer provides semantic interpretation and visualization of standard ROS2 message types, creating meaningful Rerun visualization components from robotics data. Unlike the protobuf layer, this layer understands the semantics of ROS2 messages and creates appropriate visualizations: images become Image components, point clouds become Point3D components, IMU data becomes Transform3D components, and so on.

This layer supports standard ROS2 packages including `sensor_msgs`, `geometry_msgs`, `std_msgs`, and `builtin_interfaces`. This layer provides visualization of sensor data like cameras and LiDAR with minimal setup required.

See [Message Formats](message-formats.md) for the complete list of supported message types.

### Protobuf decoding

The `protobuf` layer automatically decodes protobuf-encoded messages using reflection, creating structured component data based on the protobuf schema. Message fields become Rerun components that you can query and analyze.

However, this layer provides structured access without semantic visualization meaning. While the data becomes queryable, it won't automatically appear as meaningful visualizations like images or point clouds—it gives you the data structure, not the visual interpretation.

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
- Data from the `ros2msg` layer appears as native Rerun visualization components ([Image](../../reference/types/archetypes/image.md), [Points3D](../../reference/types/archetypes/points3d.md.md), etc.)
- Data from the `raw` layer appears as blob components containing the original message bytes
- Metadata from `schema`, `stats`, and `recording_info` layers appears as dedicated metadata entities

For more information on querying data and working with archetypes, see the [Data Queries documentation](../../howto/get-data-out.md).
