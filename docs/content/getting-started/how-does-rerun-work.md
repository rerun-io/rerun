---
title: How Rerun works
order: 500
description: The components of Rerun, how they connect, and how you use them
---

Rerun covers the whole journey from raw recordings to training, on a single unified data layer for multi-rate, multimodal robotics data. <!-- TODO(RR-6462): update this Rerun description to the new branding -->

It consists of **Rerun SDK**: an open source library and tools for logging, storing, querying, visualizing, and training on multi-rate, multimodal data; and
**[Rerun Hub](../catalog/hub.md)**: our commercial managed offering for production deployments, a data catalog and backend for large scale storage, access, and streaming of robotics data from object storage. <!-- TODO(RR-6462): update this Rerun and Rerun Hub description to the new branding -->

This page explains the components, how they connect, and how you use them at each stage, from logging to training.

## The components

### Logging SDK

The Logging SDK is how you get data into Rerun. Available for Python, Rust, and C++, it runs inside your application and logs data using [archetypes](../log/data-model/entity-component.md) — structured types like `Points3D`, `Image`, or `Transform3D`.

Data can be streamed directly to the Viewer, saved to `.rrd` files, or both.

### Viewer

The Viewer visualizes your data. It comes in two forms:

- **Native Viewer**: A desktop application for Linux, macOS, and Windows
- **Web Viewer**: A browser based application

The viewer includes a [**Chunk Store**](../log/chunks-and-rrd/chunks.md) (in-memory database for logged data) and a **gRPC endpoint** that accepts streamed data from the SDK.

The Web Viewer has performance limitations compared to the native viewer.
It runs as 32-bit Wasm, which caps memory at 4 GiB, and browsers often allow less, limiting the amount of data that can be visualized simultaneously.
It also runs single-threaded, making it generally slower than native.

Both viewers can be extended: the Native Viewer through its [Rust API](../viewer/extend/extend-ui.md), and the Web Viewer can be [embedded in web applications](../viewer/integrations/embed-web.md) or [Jupyter notebooks](../viewer/integrations/embed-notebooks.md).

### Catalog server

The catalog server provides storage and indexing for data.
Rerun Hub provides managed persistent storage, while the local open-source server indexes RRD files on disk and keeps catalog metadata in memory.
It organizes data into:

- **Datasets**: Named collections of related recordings
- **Segments**: Individual `.rrd` files registered to a dataset

The catalog server is available as:
- Open-source server for local development (`rerun server`)
- **[Rerun Hub](../catalog/hub.md)**, our commercial managed offering for production deployments <!-- TODO(RR-6462): update this Rerun Hub description to the new branding -->

### Catalog SDK

The Catalog SDK (`rerun.catalog`) is a Python library for querying and manipulating the data stored on a catalog server. Combined with Rerun Hub, it allows building complex data transformation pipelines.

## How they connect

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/0dd52f7dd14c89ab0c7c1188202ffc423b10a527_diagram-overview-dark.svg" alt="Diagram of how Rerun connects: the Logging SDK streams to the Viewer or saves .rrd files, which load into the Viewer or a catalog server">
  <img class="d2-light" src="https://static.rerun.io/0c07f9a05b29dfb52cd2d3c8f02a242adeb723f8_diagram-overview-light.svg" alt="Diagram of how Rerun connects: the Logging SDK streams to the Viewer or saves .rrd files, which load into the Viewer or a catalog server">
</div>

## Log and ingest
Use the [logging API](../log/tutorial.md) to log multimodal data from your code, or [the chunk processing API](../ingest/chunk-processing-api.md) to convert your existing data to the [.rrd](../log/chunks-and-rrd/rrd-format.md) file format to later visualize or query.
<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/d1e6392a75f1a24b19d3f8776e573466141d25e6_diagram-log-and-ingest-dark.svg" alt="Diagram of two paths into an .rrd file: a data generator writing directly, and existing data (MCAP, LeRobot, etc.) going through a converter">
  <img class="d2-light" src="https://static.rerun.io/ca2f2ff0e4db51d400c92f64b46cf794025ce1d4_diagram-log-and-ingest-light.svg" alt="Diagram of two paths into an .rrd file: a data generator writing directly, and existing data (MCAP, LeRobot, etc.) going through a converter">
</div>

### Stream to Viewer

The simplest workflow: stream data directly from your code to the Viewer for live visualization.

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/815f169557689c1bb1fe1f0d8540e837306b5837_diagram-stream-to-viewer-dark.svg" alt="Diagram of the Logging SDK streaming data directly to the Viewer">
  <img class="d2-light" src="https://static.rerun.io/204e1e7ca7c57f45a8c0005fd3f7e6b07eb37b42_diagram-stream-to-viewer-light.svg" alt="Diagram of the Logging SDK streaming data directly to the Viewer">
</div>

Minimal example:

snippet: concepts/how-does-rerun-work/log-to-grpc

Best for: development, debugging, real-time monitoring.

### Save to RRD, view later

Log data to `.rrd` files, then open them in the Viewer whenever needed. Files can be loaded from disk or URLs.

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/4192aaa12b55eb91c7de5bed266c49dedc66c4fb_diagram-save-and-load-dark.svg" alt="Diagram of the Logging SDK saving data to an .rrd file, which is then loaded into the Viewer">
  <img class="d2-light" src="https://static.rerun.io/52d6755e40c8a3327cf3c390a5395b1242f2cff9_diagram-save-and-load-light.svg" alt="Diagram of the Logging SDK saving data to an .rrd file, which is then loaded into the Viewer">
</div>

Minimal example:

snippet: concepts/how-does-rerun-work/log-to-rrd

And later:

```sh
$ rerun /tmp/my_recording.rrd
```

Best for: sharing recordings, offline analysis, archiving.

## Visualize
Rerun provides an open source pre-built [viewer](../viewer/organization.md) that is [adjustable](../viewer/inspect-recordings/configure-layout.md#panels) and [extensible](../viewer/integrations/extend.md).
You can log directly to the viewer, [open](../ingest/open-any-file.md) a range of file formats to get data into the viewer, or even connect the viewer to a Rerun [catalog](../catalog/catalog-object-model.md).

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/271a2a55bab0db15312b80f68f4b5900442b1d0e_diagram-viewer-sources-dark.svg" alt="Diagram of data sources feeding the Rerun Viewer: a data generator, .rrd files, automated importers for other formats, and the Rerun Catalog">
  <img class="d2-light" src="https://static.rerun.io/7c5d7aa3bb87ed15c53872be27fb3b7a736ae279_diagram-viewer-sources-light.svg" alt="Diagram of data sources feeding the Rerun Viewer: a data generator, .rrd files, automated importers for other formats, and the Rerun Catalog">
</div>

## Query and transform
Ask questions across all your recordings at once, down to individual sensor readings:

- Where did the robot miss a grasp?
- Which episodes dropped camera frames?
- Does the new checkpoint succeed more often than the old one?
- What was every sensor reading just before contact?

Results can link straight to the moment in the viewer.

You can use the open source [catalog](../catalog/catalog-object-model.md) server for running local [laptop scale examples](../query-and-transform/tutorial.md).
We also offer **[Rerun Hub](../catalog/hub.md)** for teams that need collaborative dataset management, version control, and cloud storage ([reach out](https://5li7zhj98k8.typeform.com/to/a5XDpBkZ?typeform-source=docs) to learn more). <!-- TODO(RR-6462): update this Rerun Hub description to the new branding -->
The two are API compatible, so to run our examples on **Rerun Hub** you connect to your Hub instead of launching a server, and register recordings from object storage instead of local paths (see [Run the examples on Hub](../catalog/hub.md#run-the-examples-on-hub)).

### Prepare catalog
Before querying or viewing recordings on a catalog server, register them.
The Viewer can also open an `.rrd` in its built-in catalog without a separate server.
We group recordings as [datasets](../catalog/catalog-object-model.md#datasets).
Since Rerun indexes existing data in place, registration needs paths to RRDs to index: in object store for **Rerun Hub** or on disk for local catalog server.

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/4be085d114fedbc5a3581751786cd9e3b0d457c2_diagram-prepare-catalog-dark.svg" alt="Diagram of preparing a catalog: create_dataset sends a dataset name, the Rerun Catalog returns a handle, handle.register sends .rrd paths">
  <img class="d2-light" src="https://static.rerun.io/57ce8623fed1c98d2673b9875c7074cef1f15034_diagram-prepare-catalog-light.svg" alt="Diagram of preparing a catalog: create_dataset sends a dataset name, the Rerun Catalog returns a handle, handle.register sends .rrd paths">
</div>

Minimal example of creating a dataset and registering files:

snippet: concepts/how-does-rerun-work/use-catalog[prepare]

Best for: large datasets, team collaboration, production pipelines.

### Use catalog
At this point a viewer can connect to the prepared catalog or we show the basic steps to perform a query.
We specify what dataset we want to query, get access to a lazy loaded [dataframe](../query-and-transform/dataframes/dataframe-queries.md), specify our query, and retrieve the results.
Queries can be specified with SQL or dataframe APIs allowing the flexibility to investigate anything about your data.

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/962b9d84bdb3a538d1bb679a6f8485a4a76a3ea5_diagram-query-catalog-dark.svg" alt="Diagram of querying the Rerun Catalog: a dataset handle returns a DataFrame, and a select and where query on it returns the query result">
  <img class="d2-light" src="https://static.rerun.io/bb93e7dc4a3c42db7a1d3cfa4499f9f0c4bb7825_diagram-query-catalog-light.svg" alt="Diagram of querying the Rerun Catalog: a dataset handle returns a DataFrame, and a select and where query on it returns the query result">
</div>

Use the Catalog SDK to query data from a catalog server, process it, and write results back. Visualization is available at any time.

Minimal example of querying a dataset:

snippet: concepts/how-does-rerun-work/use-catalog[query]

Best for: data pipelines, batch processing, ML training data preparation.

## Train
Use the catalog as a data source for [training](../train/tutorial.md): a dataloader runs a query against the catalog and yields training batches.

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/e90cccf334dc9d99259df203e0dd4cb07eb9201b_diagram-dataloader-dark.svg" alt="Diagram of a query feeding a dataloader that reads from the Rerun Catalog, which returns training batches">
  <img class="d2-light" src="https://static.rerun.io/bfd3c5988bee457ca039f62f6b23945a66d7c9ec_diagram-dataloader-light.svg" alt="Diagram of a query feeding a dataloader that reads from the Rerun Catalog, which returns training batches">
</div>

## What ships where?

### Hosted web viewer

The Web Viewer is available at [rerun.io/viewer](https://rerun.io/viewer).
It's a great place to start exploring the examples.

### CLI

The `rerun` binary bundles multiple tools in one:
- **Native Viewer** for visualization
- **Open-source server** (via `rerun server`)
- **RRD tools** for file manipulation
- **Web Viewer** (via `rerun --serve-web`)

The Rerun CLI can be downloaded from [GitHub](https://github.com/rerun-io/rerun/releases) or as part of the Python SDK.
It can also be built from source with `cargo install rerun-cli --locked`.

See: [CLI reference](../reference/cli.md)

### Python SDK

The Python SDK includes:
- **Logging SDK**
- **Catalog SDK**
- **CLI**, including the Viewer (the `rerun` CLI is made available by installing the `rerun-sdk` Python package)

See: Python SDK [installation instructions](install-rerun/python.md) and [quick start guide](../log/tutorial.md)

### Rust SDK

The Logging SDK as a Rust crate.

See: Rust SDK [installation instructions](install-rerun/rust.md) and [quick start guide](../log/tutorial.md)

### C++ SDK

The Logging SDK for C++ projects.

See: C++ SDK [installation instructions](install-rerun/cpp.md) and [quick start guide](../log/tutorial.md)

### The `web-viewer` and `web-viewer-react` NPM packages

These NPM packages bundle the Web Viewer for inclusion on a website.

See: the `web-viewer` package [reference](../reference/npm.md)
