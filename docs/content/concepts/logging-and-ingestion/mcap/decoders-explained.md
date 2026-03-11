---
title: MCAP Decoders Explained
order: 300
---

MCAP processing in Rerun uses a decoder architecture where each decoder represents a different way to interpret and extract data from the same MCAP source.
By default, when opening a file Rerun analyzes an MCAP file to determine which decoders are active to provide the most comprehensive view of your data, while avoiding duplication.
You can specify which decoders to use during conversion, allowing you to extract exactly the information you need for your analysis.

## Understanding decoders with an example

When multiple decoders are enabled, they each process the same messages independently, creating different component types on identical entity paths. This can result in data duplication—for instance, enabling both `raw` and `protobuf` decoders stores the same message as both structured field data and raw binary blobs.

Consider an MCAP file from a ROS2 robot containing sensor data on the topic `/robot/camera/image_raw` with ROS2 `sensor_msgs/msg/Image` messages:

- With only the `ros2msg` decoder: Creates an [Image](../../../reference/types/archetypes/image.md) archetype for direct visualization in Rerun's viewer
- With only the `raw` decoder: Creates an [McapMessage](../../../reference/types/archetypes/mcap_message.md) containing the original CDR-encoded message bytes
- With both decoders enabled: All representations coexist on the same entity path `/robot/camera/image_raw`

## Schema and statistics decoders

The `schema` decoder extracts structural information about the MCAP file's organization, creating metadata entities that describe channel definitions, topic names with their message types, and schema definitions. This decoder is particularly useful for understanding unfamiliar MCAP files or getting an overview of available topics and channels before deeper processing.

The `stats` decoder computes file-level metrics and statistics, creating entities with message counts per channel, temporal ranges, file size information, and data rate analysis. This gives you insight into the scale and characteristics of your dataset for quality assessment and planning storage requirements.

## Message interpretation decoders

### Semantic interpretation

The `ros2msg` and `foxglove` decoders provide semantic interpretation and visualization of standard ROS 2 and Foxglove message types, creating meaningful Rerun visualization archetypes from data. Unlike the `protobuf` decoder, this decoder understands the semantics of the messages and creates appropriate visualizations: images become [Image](../../../reference/types/archetypes/image.md), point clouds become [Points3D](../../../reference/types/archetypes/points3d.md), IMU messages become [SeriesLines](../../../reference/types/archetypes/series_lines.md) with the data plotted over time, and so on.

See [Message Formats](message-formats.md) for the complete list of supported message types.

### Protobuf decoding

The `protobuf` decoder automatically decodes protobuf-encoded messages using reflection, creating structured component data based on the protobuf schema. Message fields become Rerun components that you can query and analyze.

However, this decoder provides structured access without semantic visualization meaning. While the data becomes queryable, it won't automatically appear as meaningful visualizations like images or point clouds, it gives you the data structure, not the visual interpretation.

## The raw decoder

The `raw` decoder preserves the original message bytes without any interpretation, creating blob entities containing the unprocessed message data. Each message appears as a binary blob that can be accessed programmatically for custom analysis tools.

## Recording info

The `recording_info` decoder extracts metadata about the recording session and capture context, creating metadata entities with information about recording timestamps, source system details, and capture software versions.

## Decoder selection and performance

### Selecting decoders

By default, Rerun processes MCAP files with all decoders active. You can control which decoders are used when [converting MCAP files via the CLI](cli-reference.md) using the `-d` flag:

```bash
# Use only specific decoders
rerun mcap convert input.mcap -d protobuf -d stats -o output.rrd

# Use multiple decoders for different perspectives
rerun mcap convert input.mcap -d ros2msg -d raw -d recording_info -o output.rrd
```

## Accessing decoder data

Each decoder creates different types of components on entity paths (derived from MCAP channel topics) that can be accessed through Rerun's SDK:

- Data from the `ros2msg` decoder and supported Foxglove messages appears as native Rerun visualization archetypes (see [here](message-formats.md#overview) for an overview)
- Other data from the `protobuf` or `ros2_reflection` decoders appears as structured components that can be queried by field name or manually added to certain views ([example](message-formats.md#example-time-series-plot-for-custom-message-scalars))
- Data from the `raw` decoder appears as blob components containing the original message bytes
- Metadata from `schema`, `stats`, and `recording_info` decoders appears as dedicated metadata entities

For more information on querying data and working with archetypes, see the [Data Queries documentation](../../../howto/query-and-transform/get-data-out.md).

Each of these decoders contributes their own [chunks](../chunks.md) to the Rerun-native data.
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
| Raw message data | `mcap.Message:data`             | Unprocessed message bytes stored as binary blobs, handled by the `raw` decoder. |
