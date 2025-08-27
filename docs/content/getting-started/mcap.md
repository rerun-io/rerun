---
title: Working with MCAP files
order: 450
---

The Rerun Viewer has built-in support for opening [MCAP files](https://mcap.dev/), an open container format for storing timestamped messages.

⚠️ **This is an early version of MCAP support** that will continue to evolve and expand over time.

## How Rerun processes MCAP files

Rerun uses a _layered architecture_ to process MCAP files at different levels of abstraction. This design allows the same MCAP file to be processed in multiple ways simultaneously, from raw bytes to semantically meaningful visualizations.

Each layer extracts different types of information from the same MCAP data:

- `raw`: Logs the unprocessed message bytes as Rerun blobs without any interpretation. Useful when you need access to the original data for custom processing.

- `schema`: Extracts metadata about channels, topics, and schemas to provide an overview of the MCAP file contents. Helps you understand what data is available before processing.

- `stats`: Extracts file-level metrics like message counts, time ranges, and channel statistics. Useful for understanding the characteristics and scale of your data.

- `protobuf`: Automatically decodes protobuf-encoded messages using reflection. Creates structured data that's queryable but doesn't provide semantic meaning for visualization.

- `ros2msg`: Provides semantic conversion of common ROS2 message types (sensor messages, geometry messages, diagnostic messages) into Rerun's visualization components. Converts things like sensor data, images, point clouds, poses, transforms, and paths into formats the viewer can display meaningfully.

- `recording_info`: Extracts recording metadata and session information to provide context about when and how the data was captured.

By default, Rerun processes MCAP files with all layers active to provide the most comprehensive view of your data. However, you can also choose to activate only specific layers that are relevant to your use case. For example, you might only want the raw data and statistics, or just the ROS2 semantic layer for visualization.

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
