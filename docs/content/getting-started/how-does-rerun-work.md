---
title: How Rerun works
order: 500
description: The components, how they connect, and how you use them from logging to training
---

Rerun covers the whole journey from raw recordings to training, on a single unified data layer for multi-rate, multimodal robotics data.

It's comprised of **Rerun SDK**: an open source library and tools for logging, storing, querying, visualizing, and training on multi-rate, multimodal data; and
**Rerun Hub**: a data catalog and backend for large scale storage, access, and streaming of robotics data from object storage.

This page explains the components, how they connect, and how you use them at each stage, from logging to training.

## The components

### Logging SDK

The Logging SDK is how you get data into Rerun. Available for Python, Rust, and C++, it runs inside your application and logs data using [archetypes](../logging-and-ingestion/data-model/entity-component.md) — structured types like `Points3D`, `Image`, or `Transform3D`.

Data can be streamed directly to the Viewer, saved to `.rrd` files, or both.

### Viewer

The Viewer visualizes your data. It comes in two forms:

- **Native Viewer**: A desktop application for Linux, macOS, and Windows
- **Web Viewer**: A browser based application

The viewer includes a [**Chunk Store**](../logging-and-ingestion/chunks-and-rrd/chunks.md) (in-memory database for logged data) and a **gRPC endpoint** that accepts streamed data from the SDK.

The Web Viewer has performance limitations compared to the native viewer. It runs as 32-bit Wasm and is limited to ~2 GiB memory in practice, limiting the amount of data that can be visualized simultaneously. It also runs single-threaded, making it generally slower than native.

Both viewers can be extended: the Native Viewer through its [Rust API](../viewer/extend/extend-ui.md), and the Web Viewer can be [embedded in web applications](../integrations/embed-web.md) or [Jupyter notebooks](../integrations/embed-notebooks.md).

### Catalog server

The catalog server provides storage and indexing for data.
Rerun Hub provides managed persistent storage, while the local open-source server indexes RRD files on disk and keeps catalog metadata in memory.
It organizes data into:

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

## Log and ingest
Use the [logging API](../logging-and-ingestion/tutorial.md) to log multimodal data from your code, or [the chunk processing API](../logging-and-ingestion/import/chunk-processing-api.md) to convert your existing data to the [.rrd](../logging-and-ingestion/chunks-and-rrd/rrd-format.md) file format to later visualize or query.
<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/d1c242b745000b3dbba0dc42a861e2e6b760d614_d2.svg" alt="">
  <img class="d2-light" src="https://static.rerun.io/0aa33c2c86855ee06122e761562ca31e82486ad6_d2-light.svg" alt="">
</div>

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

## Visualize
Rerun provides an open source pre-built [viewer](../viewer/ui/overview.md) that is [adjustable](../viewer/tutorial.md) and [extensible](../integrations/extend.md).
You can log directly to the viewer, [open](../logging-and-ingestion/import/open-any-file.md) a range of file formats to get data into the viewer, or even connect the viewer to a Rerun [catalog](../query-and-transform/catalog/catalog-object-model.md).

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/2ded478d0e3d66b8532f1b9991ed786a2919d6d7_d2.svg" alt="">
  <img class="d2-light" src="https://static.rerun.io/b7baf766ffcb27babd2866ebc05f016e8da53111_d2-light.svg" alt="">
</div>

## Query and transform
Ask questions across all your recordings at once, down to individual sensor readings:

- Where did the robot miss a grasp?
- Which episodes dropped camera frames?
- Does the new checkpoint succeed more often than the old one?
- What was every sensor reading just before contact?

Results can link straight to the moment in the viewer.

You can use the open source [catalog](../query-and-transform/catalog/catalog-object-model.md) server for running local [laptop scale examples](../query-and-transform/tutorial.md).
We also offer **Rerun Hub**, a scalable catalog for robotic data, for teams that need collaborative dataset management, version control, and cloud storage ([reach out](https://5li7zhj98k8.typeform.com/to/a5XDpBkZ?typeform-source=docs) to learn more).
These are API compatible so the only difference from our examples to **Rerun Hub** is that you connect to an existing server instead of launching your own.

### Prepare catalog
Before querying or viewing recordings on the catalog we have to register them.
We group recordings as [datasets](../query-and-transform/catalog/catalog-object-model.md#datasets).
Since Rerun indexes existing data in place, registration needs paths to RRDs to index: in object store for **Rerun Hub** or on disk for local catalog server.

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/40ada7a9f3834208554b8de80a0bab3fb8e5f108_d2.svg" alt="">
  <img class="d2-light" src="https://static.rerun.io/63998cd168884dbe92c585650d2d8cdcdff826fc_d2-light.svg" alt="">
</div>

Minimal example of creating a dataset and registering files:

```python
import rerun as rr

client = rr.catalog.CatalogClient("rerun://example.cloud.rerun.io")
dataset = client.create_dataset("my_data")
dataset.register(["s3://my-rrd-files/recording1.rrd", "s3://my-rrd-files/recording2.rrd"])
```

Best for: large datasets, team collaboration, production pipelines.

### Use catalog
At this point a viewer can connect to the prepared catalog or we show the basic steps to perform a query.
We specify what dataset we want to query, get access to a lazy loaded [dataframe](../query-and-transform/dataframes/dataframe-queries.md), specify our query, and retrieve the results.
Queries can be specified with SQL or dataframe APIs allowing the flexibility to investigate anything about your data.

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/4bfd86b4605405d018ecd44d8d8062114126a952_d2.svg" alt="">
  <img class="d2-light" src="https://static.rerun.io/6922fdbb9c24357ea3a585be62d6ccb572a70350_d2-light.svg" alt="">
</div>

Use the Catalog SDK to query data from a catalog server, process it, and write results back. Visualization is available at any time.

Minimal example of querying a dataset:

```python
import datafusion as dfn
import rerun as rr

client = rr.catalog.CatalogClient("rerun://example.cloud.rerun.io")
dataset = client.get_dataset("my_data")
df = dataset.filter_contents("/obs").reader(index="log_time")  # `df` is a DataFusion dataframe
df.filter(dfn.col("/obs:Scalars:scalars").is_not_null()).count()  # count observations in recording
```

Best for: data pipelines, batch processing, ML training data preparation.

## Train
Use the catalog as a data source for [training](../train/tutorial.md): a dataloader runs a query against the catalog and yields training batches.

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/15a375cfcc03a73d74acdec06b5f36c43e988dfe_d2-dark.svg" alt="">
  <img class="d2-light" src="https://static.rerun.io/75c7bcab918f7067cae09eb665cdec4d3d466e70_d2-light.svg" alt="">
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

See: Python SDK [installation instructions](install-rerun/python.md) and [quick start guide](../logging-and-ingestion/tutorial.md)

### Rust SDK

The Logging SDK as a Rust crate.

See: Rust SDK [installation instructions](install-rerun/rust.md) and [quick start guide](../logging-and-ingestion/tutorial.md)

### C++ SDK

The Logging SDK for C++ projects.

See: C++ SDK [installation instructions](install-rerun/cpp.md) and [quick start guide](../logging-and-ingestion/tutorial.md)

### The `web-viewer` and `web-viewer-react` NPM packages

These NPM packages bundle the Web Viewer for inclusion on a website.

See: the `web-viewer` package [reference](../reference/npm.md)
