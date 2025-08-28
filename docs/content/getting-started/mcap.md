---
title: Working with MCAP files
order: 450
---

The Rerun Viewer has built-in support for opening [MCAP files](https://mcap.dev/), an open container format for storing timestamped messages.

⚠️ **This is an early version of MCAP support** that will continue to evolve and expand over time. We are actively seeking feedback from the community to guide development priorities. Reinterpretation of custom messages and enhanced query capabilities are planned for following releases.

## How Rerun processes MCAP files

Rerun uses a _layered architecture_ to process MCAP files at different levels of abstraction. This design allows the same MCAP file to be processed in multiple ways simultaneously, from raw bytes to semantically meaningful visualizations.

Each layer extracts different types of information from the same MCAP data:

- `raw`: Logs the unprocessed message bytes as Rerun blobs without any interpretation. Useful when you need access to the original data for custom processing.

- `schema`: Extracts metadata about channels, topics, and schemas to provide an overview of the MCAP file contents. Helps you understand what data is available before processing.

- `stats`: Extracts file-level metrics like message counts, time ranges, and channel statistics. Useful for understanding the characteristics and scale of your data.

- `protobuf`: Automatically decodes protobuf-encoded messages using reflection. Creates structured data that's queryable but doesn't provide semantic meaning for visualization.

- `ros2msg`: Provides semantic conversion of common ROS2 message types into Rerun's visualization components. Supports messages from standard ROS2 packages including `sensor_msgs` (images, point clouds, IMU data, camera info), `geometry_msgs`, `std_msgs`, and others. See the [complete list of supported message types](https://github.com/rerun-io/rerun/tree/main/crates/utils/re_mcap/src/parsers/ros2msg) for details.

- `recording_info`: Extracts recording metadata and session information to provide context about when and how the data was captured.

By default, Rerun processes MCAP files with all layers active to provide the most comprehensive view of your data. All layers are available whether you're directly loading an MCAP file into the viewer or converting to RRD format first. However, you can also choose to activate only specific layers that are relevant to your use case.

When multiple layers are active, each layer's data is stored separately in the resulting dataset. This means that including both `protobuf` and `ros2msg` layers will store data from both interpretations, which increases storage size but provides different views of the same underlying messages.

You can control which layers are used when [converting MCAP files via the CLI](../../reference/cli.md#rerun-mcap) using the `-l` flag to specify individual layers. Multiple layers can work on the same data simultaneously, each providing their own perspective on the content.

## Opening MCAP files

The Viewer can load MCAP files in 3 different ways:

- via CLI arguments (e.g. `rerun data.mcap`),
- using drag-and-drop,
- using the open dialog in the Rerun Viewer.

All these file loading methods support loading a single file, many files at once (e.g. `rerun myfiles/*`), or even folders.

For more information about loading files in general, see [Opening files](./data-in/open-any-file.md).

## Using the CLI

You can view an MCAP file directly:

```bash
rerun input.mcap
```

Or convert an MCAP file to Rerun's native RRD format:

```bash
rerun mcap convert input.mcap -o output.rrd
```

You can specify which layers to extract during conversion:

```bash
rerun mcap convert input.mcap -l protobuf -l stats -o output.rrd
```

For more details on the available commands and options, see the [`rerun mcap` reference](../../reference/cli.md#rerun-mcap).

## Message format support

### ROS2 Messages

Rerun provides semantic interpretation for a growing set of standard ROS2 message types. Currently supported packages include:

- **`sensor_msgs`**: Images, compressed images, point clouds, IMU data, camera info, joint states
- **`std_msgs`**: Basic data types like strings and headers
- **`geometry_msgs`**: Poses, transforms, and geometric data
- **`builtin_interfaces`**: Time and duration types

We will be continually adding support for more standard messages. Please file a [GitHub issue](https://github.com/rerun-io/rerun/issues) or mention on our [Discord](https://discord.gg/PXtCgFBSmH) any feedback or something you'd like added.

### ROS1 Messages

ROS1 messages are not currently supported for semantic interpretation through the `ros2msg` layer. However, ROS1 MCAP files can still be processed using the `raw`, `schema`, `stats`, and `recording_info` layers to access the underlying data and metadata.

### Other Message Formats

Messages encoded with protobuf can be automatically decoded using the `protobuf` layer, which provides structured access to fields without semantic interpretation for visualization.

## Working with MCAP data

### In the Viewer

When you open an MCAP file in the Rerun Viewer, the layer processing happens automatically. Data from different layers appears as separate entity paths in the entity tree, allowing you to explore both raw message data and semantically interpreted visualizations side by side.

The layer structure is visible in the viewer's entity hierarchy - for example, you might see paths like `/topic_name` for semantic data from the `ros2msg` layer, alongside metadata entities for schema and statistics information.

### Querying Data

Once MCAP data is loaded (either directly or via RRD), you can query it using Rerun's standard data access patterns. The data becomes part of Rerun's temporal database, indexed by the multiple timeline dimensions extracted during processing (log time, publish time, and any additional sensor timestamps).

Currently, the query interface works with the processed Rerun component data rather than being layer-aware. This means you work with the final interpreted data (images, point clouds, etc.) rather than the original message structure.
