---
title: Working with MCAP
order: 500
---

The Rerun Viewer has built-in support for opening [MCAP](https://mcap.dev/) files, an open container format for storing timestamped messages.

⚠️ **This is an early version of MCAP support** that will continue to evolve and expand over time. We are actively seeking feedback from the community to guide development priorities. Reinterpretation of custom messages and enhanced query capabilities are planned for following releases.

## Quick start

### Loading MCAP files

The simplest way to get started is to load an MCAP file directly:

```bash
# View an MCAP file in the Rerun Viewer
rerun your_data.mcap
```

You can also drag and drop MCAP files into the Rerun Viewer

### Basic conversion

Convert MCAP files to Rerun's native format for faster loading:

```bash
# Convert MCAP to RRD format for faster loading
rerun mcap convert input.mcap -o output.rrd

# View the converted file
rerun output.rrd
```

## Layered architecture

Rerun uses a _layered architecture_ to process MCAP files at different levels of abstraction. This design allows the same MCAP file to be processed in multiple ways simultaneously, from raw bytes to semantically meaningful visualizations.

Each layer extracts different types of information from the same MCAP data:

- **`raw`**: Logs the unprocessed message bytes as Rerun blobs without any interpretation
- **`schema`**: Extracts metadata about channels, topics, and schemas
- **`stats`**: Extracts file-level metrics like message counts, time ranges, and channel statistics
- **`protobuf`**: Automatically decodes protobuf-encoded messages using reflection
- **`ros2msg`**: Provides semantic conversion of common ROS2 message types into Rerun's visualization components
- **`recording_info`**: Extracts recording metadata and session information

By default, Rerun processes MCAP files with all layers active to provide the most comprehensive view of your data. You can also choose to activate only specific layers that are relevant to your use case.

For a detailed explanation of how each layer works and when to use them, see [Layers Explained](mcap/layers-explained.md).

## Supported message formats

Rerun provides automatic visualization for common ROS2 message types, ROS1 message types are not currently supported for semantic interpretation through any layer.

Protobuf messages are automatically decoded for structured access, while unsupported message types remain available as raw bytes.

For more details about all supported message types, see [Message Formats](mcap/message-formats.md).

## Advanced usage

For advanced command-line options and automation workflows, see the [CLI Reference](mcap/cli-reference.md) for complete documentation of all available commands and flags.
