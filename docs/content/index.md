---
title: Documentation
order: 0
---

## Use Rerun to build intelligent software faster

Rerun helps developers debug and understand their systems
by quickly visualizing internal state and data.

It's primarily focused on visualizing 2D and 3D computer vision and robotics data over time,
but is under active development to expand support for more use-cases and datatypes.

To get a sense of what you can do with Rerun right now,
check out the [example gallery](/examples)
or try out some [live demos](https://demo.rerun.io/) directly in your browser.

## Start learning

- Jump right in with [Python](getting-started/python.md) or [Rust](getting-started/rust.md).
- Follow up with a [walkthrough of the viewer](getting-started/viewer-walkthrough.md) and a tutorial on logging data with [Python](getting-started/logging-python.md) or [Rust](getting-started/logging-rust.md).
- Dive deeper in the [Concepts](concepts) section.
- The [Reference](reference) covers datatypes, configuration and API details.
    - Including simple examples of how to use each of the [loggable data types](reference/data_types)

## How does it work?
It's quite simple:

1. Use the Rerun SDK to log data like text, tensors, images, point clouds, or metrics.
2. The data you log gets sent to our viewer that automatically visualizes it live.
3. You can then use the UI to interactively explore the data and customize layout and visualization options.
4. Save recordings to file for later replay.
5. You can also [extend Rerun in Rust](howto/extend-ui) to meet your specific needs.

Under the hood Rerun:

1. Serializes the data.
2. Sends it to the viewer, on the same machine or across the network.
3. Receives and deserializes the data, potentially coming in out-of-order from from multiple sources.
4. Indexes it into our super fast in-memory time-series like database.
5. Serves and renders that data at lightning speed as you interactively inspect and scroll back and forth in time.

### Example Visualization
![overview](https://static.rerun.io/9a555db43ccdc24a5a0d9afb3e9bf5c80b55f271_docs_overview.png)

## Can't find what you're looking for?

- Drop in to the [Rerun Community Discord](https://discord.gg/xwcxHUjD35)
- Or [submit an issue](https://github.com/rerun-io/rerun/issues) in the Rerun GitHub project

