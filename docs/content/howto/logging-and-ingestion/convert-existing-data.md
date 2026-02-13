---
title: Convert existing data to Rerun
order: 200
description: How to convert custom data formats to Rerun using row-oriented or columnar APIs
---

There are a variety of ways to convert data into an `RRD`.
When filetypes are opened in the viewer they go through our [dataloaders](../../concepts/logging-and-ingestion/data-loaders.md).

For example, there's a built-in dataloader for [MCAP files](../../concepts/logging-and-ingestion/mcap.md) and we also have a few [command line options](../../concepts/logging-and-ingestion/mcap/cli-reference.md) for converting MCAP data directly into an `RRD`.
This works great for message types that are supported by the built-in dataloader - however, the most general solution to support arbitrary message types is the logging API.

---

Other relevant tutorials:

-   [Log and Ingest](../../getting-started/data-in.md)
-   [Send entire columns at once](send-columns.md)
-   [Working with MCAP](../../howto/logging-and-ingestion/mcap.md)

## Converting existing data to RRD
This guide covers the two recommended approaches: `recording.log` (row-oriented) and `recording.send_columns` (columnar). Both produce identical `.rrd` output.

## Quick comparison

Rerun offers two APIs that we will use for conversion. Both produce identical `.rrd` files:

| | `recording.log` | `recording.send_columns` |
|---|---|---|
| **API style** | Row-oriented: one entity per call | Columnar: many timestamps per call |
| **Best for** | Live streaming, prototyping, simple conversions | Batch conversion of large datasets |
| **Performance** | Lower throughput, no batch latency | ~3–10x faster for batch workloads |
| **Typical use cases** | Sensor streams, simple scripts | Bulk data conversion |
| **Language support** | Python, Rust, C++ | Python, Rust, C++ |


## When to use which

**Use `recording.log` when:**

* Your dataset is small and performance isn't critical
* Implementation simplicity is the priority

**Use `recording.send_columns` when:**

* You're doing batch conversion of large recorded datasets
* You have high-frequency signals (transforms, IMU, joint states)

Here are timings from a real-world MCAP conversion with custom Protobuf messages (~21k messages total):

| | `recording.log` | `recording.send_columns` |
|---|---|---|
| Video frames (2,363 msgs) | 0.12s | 0.01s |
| Transforms (16,505 msgs) | 0.84s | 0.08s |
| Other messages (2,354 msgs) | 0.09s | 0.01s |
| **Total Rerun logging time** | **1.33s** | **0.10s** |

> **Note:** These are example timings from a specific dataset. Actual performance will vary. The relative speedup (10-13x here) is typical for the Rerun logging step of batch conversions.

## Map to archetypes

Regardless of which API you use, the goal is to map your custom data into Rerun [archetypes](../../reference/types/archetypes.md).

When writing your converter, the first question for each message type is: **What is the proper Rerun archetype?**

* For example, transforms and poses map to [`Transform3D`](../../reference/types/archetypes/transform3d.md) and [`InstancePoses3D`](../../reference/types/archetypes/instance_poses3d.md), an image to [`Image`](../../reference/types/archetypes/image.md), point clouds to [`Points3D`](../../reference/types/archetypes/points3d.md).
* For data that does not map cleanly to existing Archetypes, you can use [`AnyValues`](custom-data.md) for simple key-value pairs, or [`DynamicArchetype`](custom-data.md) when you want to group related fields under a named archetype.
Both appear in the dataframe view and are queryable, but don't specify visual qualities as explicitly.

## Converter structure with `recording.log`

**Full working example:** [Converting MCAP Protobuf data using `recording.log`](https://github.com/rerun-io/rerun/blob/main/docs/snippets/all/howto/convert_mcap_protobuf.py)

Here is an example of how you could build a converter using `log` calls.
We use handler functions for each message type we want to convert.
Each handler sets timestamps and logs directly.

First, we add an utility to manage logging timestamps:

snippet: howto/convert_mcap_protobuf[set_message_times]

Then we specify how to convert specific kinds of messages:

snippet: howto/convert_mcap_protobuf[compressed_video]

Finally, we loop over all messages and log them:

snippet: howto/convert_mcap_protobuf[conversion_loop]


## Converter structure with `recording.send_columns`

**Full working example:** [Converting MCAP Protobuf data using `recording.send_columns`](https://github.com/rerun-io/rerun/blob/main/docs/snippets/all/howto/convert_mcap_protobuf_send_column.py)

Our example for `send_columns` works differently because it sends data in batches instead of single log calls.
For this purpose, our handlers first extract data into collector utilities.
These collectors first accumulate data during iteration and then send it in bulk after the loop.

**Note:** the `ColumnCollector` used below is a user-defined helper class (not part of the Rerun SDK) that accumulates time-indexed data and sends it via `send_columns`.
See the full example for its implementation.

snippet: howto/convert_mcap_protobuf_send_column[conversion_loop]

