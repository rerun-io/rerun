---
title: How does Rerun work?
order: 0
description: The high-level architecture and how the pieces fit together
---

Rerun has several components manage multimodal data across its lifetime. This page explains what they are and how they connect.

## The components

### Logging SDK

The Logging SDK is how you get data into Rerun. Available for Python, Rust, and C++, it runs inside your application and logs data using [archetypes](logging-and-ingestion/entity-component.md) — structured types like `Points3D`, `Image`, or `Transform3D`.

Data can be streamed directly to the Viewer, saved to `.rrd` files, or both.

### Viewer

The Viewer visualizes your data. It comes in two forms:

- **Native Viewer**: A desktop application for Linux, macOS, and Windows
- **Web Viewer**: A browser based application

The viewer includes a [**Chunk Store**](logging-and-ingestion/chunks.md) (in-memory database for logged data) and a **gRPC endpoint** that accepts streamed data from the SDK.

The Web Viewer has performance limitations compared to the native viewer. It runs as 32-bit Wasm and is limited to ~2 GiB memory in practice, limiting the amount of data that can be visualized simultaneously. It also runs single-threaded, making it generally slower than native.

Both viewers can be extended: the Native Viewer through its [Rust API](../howto/visualization/extend-ui.md), and the Web Viewer can be [embedded in web applications](../howto/integrations/embed-web.md) or [Jupyter notebooks](../howto/integrations/embed-notebooks.md).

### Catalog server

The catalog server provides persistent storage and indexing for large-scale data. It organizes data into:

- **Datasets**: Named collections of related recordings
- **Segments**: Individual `.rrd` files registered to a dataset

Data is served via the **redap** protocol (**Re**run **Da**ta **P**rotocol).

The catalog server is available as:
- Open-source server for local development (`rerun server`)
- **Rerun Hub**, our managed offering for production deployments

### Catalog SDK

The Catalog SDK (`rerun.catalog`) is a Python library for querying and manipulating the data stored on a catalog server. Combined with Rerun Hub, it allows building complex data transformation pipelines.


## How they connect

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/d28ca214a6a6e8386b76e6d9a841901a00051a83_d2.svg" alt="">
  <img class="d2-light" src="https://static.rerun.io/5ed885f3c2319cad1fe6f6ba55b68ac88ce989d8_d2-light.svg" alt="">
</div>


## What ships where?

### Hosted web viewer

The Web Viewer is available at [rerun.io/viewer](https://rerun.io/viewer).
It's a great place to start exploring the examples.

### CLI

The `rerun` binary bundles multiple tools in one:
- **Native Viewer** for visualization
- **OSS catalog server** (via `rerun server`)
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

See: Python SDK [installation instructions](../getting-started/install-rerun/python.md) and [quick start guide](../getting-started/data-in.md)

### Rust SDK

The Logging SDK as a Rust crate.

See: Rust SDK [installation instructions](../getting-started/install-rerun/rust.md) and [quick start guide](../getting-started/data-in.md)

### C++ SDK

The Logging SDK for C++ projects.

See: C++ SDK [installation instructions](../getting-started/install-rerun/cpp.md) and [quick start guide](../getting-started/data-in.md)

### The `web-viewer` and `web-viewer-react` NPM packages

These NPM packages bundle the Web Viewer for inclusion on a website.

See: the `web-viewer` package [reference](../reference/npm.md)

## Common workflows

### Stream to Viewer

The simplest workflow: stream data directly from your code to the Viewer for live visualization.

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/c5379695876baa96bbe0449d5f85d48e33756cf7_d2.svg" alt="">
  <img class="d2-light" src="https://static.rerun.io/f90c1b537583a0fb7c3a860aa07aa355711d04de_d2-light.svg" alt="">
</div>

Minimal example:

snippet: concepts/how-does-rerun-work/log-to-grpc

Best for: development, debugging, real-time monitoring.



### Save to RRD, view later

Log data to `.rrd` files, then open them in the Viewer whenever needed. Files can be loaded from disk or URLs.

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/f7a409bc10f2836d5e3e7b87720170a6e5bd4e46_d2.svg" alt="">
  <img class="d2-light" src="https://static.rerun.io/7529efba5c4ebb304fb0c7732ded6d22a3921084_d2-light.svg" alt="">
</div>

Minimal example:

snippet: concepts/how-does-rerun-work/log-to-rrd

And later:

```sh
$ rerun /tmp/my_recording.rrd
```

Best for: sharing recordings, offline analysis, archiving.



### Store on a catalog server

Register `.rrd` files with a catalog server for persistent, indexed storage. Query and visualize on demand.

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/18348c4200f019117478a602f6413d2603e29cbb_d2.svg" alt="">
  <img class="d2-light" src="https://static.rerun.io/0bc4ab0773cc2c68c5a0c4ee92620b819a12dea1_d2-light.svg" alt="">
</div>

Minimal example of creating a dataset and registering files:

```python
import rerun as rr

client = rr.catalog.CatalogClient("rerun://example.cloud.rerun.io")
dataset = client.create_dataset("my_data")
dataset.register(["s3://my-rrd-files/recording1.rrd", "s3://my-rrd-files/recording2.rrd"])
```


Best for: large datasets, team collaboration, production pipelines.



### Query and transform data

Use the Catalog SDK to query data from a catalog server, process it, and write results back. Visualization is available at any time.

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/7c8a7ac0667fd6395b7715fe1e5732153774ba1f_d2.svg" alt="">
  <img class="d2-light" src="https://static.rerun.io/2d2cb8f1eb42a30600ac2060c653f5a50f4c0f33_d2-light.svg" alt="">
</div>

Minimal example of querying a dataset:

```python
import datafusion as dfn
import rerun as rr

client = rr.catalog.CatalogClient("rerun://example.cloud.rerun.io")
dataset = client.get_dataset("my_data")
df = dataset.filter_contents("/obs").reader(index="log_time")  # `df` is a DataFusion dataframe
df.filter(dfn.col("obs:Scalars:scalars").is_not_null()).count()  # count observations in recording
```

Best for: data pipelines, batch processing, ML training data preparation.

