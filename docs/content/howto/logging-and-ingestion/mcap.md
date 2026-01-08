---
title: Working with MCAP
order: 700
---

The Rerun Viewer has built-in support for opening [MCAP](https://mcap.dev/) files, an open container format for storing timestamped messages.

⚠️ **This is an early version of MCAP support** that will continue to evolve and expand over time. We are actively seeking [feedback](https://rerun.io/feedback) from the community to guide development priorities. Reinterpretation of custom messages and enhanced query capabilities are planned for following releases.

## Quick start

### Loading MCAP files

The simplest way to get started is to load an MCAP file directly:

```bash
# View an MCAP file in the Rerun Viewer
rerun your_data.mcap
```

You can also drag and drop MCAP files into the Rerun Viewer or load them using the SDK:

snippet: howto/load_mcap

### Basic conversion

Convert MCAP files to Rerun's native format for faster loading:

```bash
# Convert MCAP to RRD format for faster loading
rerun mcap convert input.mcap -o output.rrd

# View the converted file
rerun output.rrd
```

## Data model

Rerun's data model is based on an [entity component system (ECS)](../concepts/entity-component.md) that is a bit different to the message-based model of [MCAP](https://mcap.dev).
To map MCAP messages to Rerun entities we make the following assumptions:

* MCAP topics corresponds to Rerun entities.
* Messages from the same topic within an MCAP chunk will be placed into a corresponding [Rerun chunk](../concepts/chunks.md).
* The contents of an MCAP message will be extracted to Rerun components and grouped under a corresponding Rerun archetype.
* `message_log_time` and `message_publish_time` of an MCAP message will be carried over to Rerun as two distinct [timelines](../concepts/timelines.md).

### Layered architecture

Rerun uses a _layered architecture_ to process MCAP files at different levels of abstraction. This design allows the same MCAP file to be ingested in multiple ways simultaneously, from raw bytes to semantically meaningful visualizations.

Each layer extracts different types of information from the MCAP source and each of the following layers will create distinct Rerun archetypes:

- **`raw`**: Logs the unprocessed message bytes as Rerun blobs without any interpretation
- **`schema`**: Extracts metadata about channels, topics, and schemas
- **`stats`**: Extracts file-level metrics like message counts, time ranges, and channel statistics
- **`protobuf`**: Automatically decodes protobuf-encoded messages using reflection
- **`ros2msg`**: Provides semantic conversion of common ROS2 message types into Rerun's visualization components
- **`ros2_reflection`**: Automatically decodes ROS2 messages using reflection
- **`recording_info`**: Extracts recording metadata such as message counts, start time, and session information

By default, Rerun analyzes an MCAP file to determine which layers are active to provide the most comprehensive view of your data, while avoiding duplication.
You can also choose to activate only specific layers that are relevant to your use case.

The following shows how to select specific layers:

```sh
# Use only specific layers
rerun mcap convert input.mcap -l protobuf -l stats -o output.rrd

# Use multiple layers for different perspectives
rerun mcap convert input.mcap -l ros2msg -l raw -l recording_info -o output.rrd
```

For a detailed explanation of how each layer works and when to use them, see [Layers Explained](../reference/mcap/layers-explained.md).

## Supported message formats

Rerun provides automatic visualization for common ROS2 message types. Protobuf messages are automatically decoded into Arrow structs, but for now will only show up in the selection panel and in the dataframe view. The contents of these MCAP files can also be queried using the Dataframe API.

Unsupported message types (such as ROS1 messages) remain available as raw bytes in Arrow format.

The following is a screenshot of the selection panel and shows a Protobuf-encoded MCAP message. The top-level fields of the Protobuf message are imported as components in the corresponding point cloud archetype. The raw MCAP schema and message information show up as separate archetypes as well.

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/mcap_raw_arrow/17b7723690c46901d14e6c1d264298ce0ca8c3ae/full.png" alt="Screenshot of MCAP messages converted to raw Arrow data in the selection panel">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/mcap_raw_arrow/17b7723690c46901d14e6c1d264298ce0ca8c3ae/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/mcap_raw_arrow/17b7723690c46901d14e6c1d264298ce0ca8c3ae/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/mcap_raw_arrow/17b7723690c46901d14e6c1d264298ce0ca8c3ae/1024w.png">
</picture>

For more details about all supported message types, see [Message Formats](../reference/mcap/message-formats.md).

## Advanced usage

For advanced command-line options and automation workflows, see the [CLI Reference](../reference/mcap/cli-reference.md) for complete documentation of all available commands and flags.
