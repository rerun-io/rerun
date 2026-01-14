---
title: How does Rerun work?
order: 0
---

Rerun has several components that work together to log, store, and visualize multimodal data. This page explains what they are and how they connect.

## The components

### Logging SDK

The Logging SDK is how you get data into Rerun. Available for Python, Rust, and C++, it runs inside your application and logs data using [archetypes](logging-and-ingestion/entity-component.md)—structured types like `Points3D`, `Image`, or `Transform3D`.

Data can be streamed directly to the Viewer, saved to `.rrd` files, or both.

### Viewer

The Viewer visualizes your data. It comes in two forms:

- **Native Viewer**: A desktop application for Linux, macOS, and Windows
- **Web Viewer**: A Wasm application that runs in browsers

The viewer includes a [**Chunk Store**](logging-and-ingestion/chunks) (in-memory database for logged data) and a **gRPC endpoint** that accepts streamed data from the SDK.

The Web Viewer has performance limitations compared to the native viewer. It runs as 32-bit Wasm and is limited to ~2 GiB memory in practice, limiting the amount of data that can be visualized simultaneously. It also runs single-threaded, making it generally slower than native.

Both viewers can be extended: the Native Viewer through its [Rust API](../howto/visualization/extend-ui.md), and the Web Viewer can be [embedded in web applications](../howto/integrations/embed-web.md) or [Jupyter notebooks](../howto/integrations/embed-notebooks.md).

### Data Platform

The Data Platform provides persistent storage and indexing for large-scale data. It organizes data into:

- **Datasets**: Named collections of related recordings
- **Segments**: Individual `.rrd` files registered to a dataset

Data is served via the **redap** protocol (Rerun Data Protocol).

The Data Platform is available as:
- Open-source server for local development (`rerun server`)
- Managed offering for production deployments

### Catalog SDK

The Catalog SDK (`rerun.catalog`) is a Python library for querying and manipulating the data stored on the Data Platform. Combined with the managed Data Platform, it allows building complex data transformation pipelines.


## How they connect

```d2
direction: down
horizontal-gap: 0
vertical-gap: 0

Logging SDK

".rrd files"

Viewer: {
  label.near: bottom-center
    
  gRPC endpoint
  Chunk Store
  Renderer
}

Viewer.gRPC endpoint -> Viewer.Chunk Store
Viewer.Chunk Store -> Viewer.Renderer

Data Platform: {
  label.near: bottom-center
  Datasets
}

Catalog SDK

Logging SDK -> Viewer.gRPC endpoint: stream
Logging SDK -> ".rrd files": save
".rrd files" -> Viewer.Chunk Store: load
".rrd files" -> Data Platform: register
Data Platform -> Viewer.Chunk Store: redap
Data Platform -> Catalog SDK: redap
```


## What ships where?

### Hosted web viewer

The Web Viewer is available at [rerun.io/viewer](https://rerun.io/viewer).
It's a great place to start exploring the examples.

### CLI

The `rerun` binary bundles multiple tools in one:
- **Native Viewer** for visualization
- **OSS Data Platform** server (via `rerun server`)
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

See: Python SDK [installation instructions](../overview/installing-rerun/python.md) and [quick start guide](../getting-started/quick-start/python.md)

### Rust SDK

The Logging SDK as a Rust crate.

See: Rust SDK [installation instructions](../overview/installing-rerun/rust.md) and [quick start guide](../getting-started/quick-start/rust.md)

### C++ SDK

The Logging SDK for C++ projects.

See: C++ SDK [installation instructions](../overview/installing-rerun/cpp.md) and [quick start guide](../getting-started/quick-start/cpp.md)

### The `web-viewer` and `web-viewer-react` NPM packages

These NPM packages bundle the Web Viewer for inclusion on a website.

See: the `web-viewer` package [reference](../reference/npm)

## Common workflows

### Stream to Viewer

The simplest workflow: stream data directly from your code to the Viewer for live visualization.

```d2
direction: right
Logging SDK -> Viewer: stream
```

Minimal example:

```python
import rerun as rr
rr.init("my_app")
rr.spawn()  # Start viewer and connect
rr.log("points", rr.Points3D([[0, 0, 0], [1, 1, 1]]))
```

Best for: development, debugging, real-time monitoring.



### Save to RRD, view later

Log data to `.rrd` files, then open them in the Viewer whenever needed. Files can be loaded from disk or URLs.

```d2
direction: right
Logging SDK -> ".rrd" -> Viewer: load
```

Minimal example:

```python
import rerun as rr
rr.init("my_app")
rr.save("recording.rrd")
rr.log("points", rr.Points3D([[0, 0, 0], [1, 1, 1]]))
```

And later:

```sh
$ rerun recording.rrd
```

Best for: sharing recordings, offline analysis, archiving.



### Store on Data Platform

Register `.rrd` files with the Data Platform for persistent, indexed storage. Query and visualize on demand.

```d2
direction: right
".rrd" -> Data Platform: register
Data Platform -> Viewer: redap
Data Platform -> Catalog SDK: redap
```

Minimal example of creating a dataset and registering files:

```python
import rerun as rr
client = rr.catalog.CatalogClient("rerun://example.cloud.rerun.io")
dataset = client.create_dataset("my_data")
dataset.register(["s3://my-rrd-files/recording1.rrd", "s3://my-rrd-files/recording2.rrd"])
```


Best for: large datasets, team collaboration, production pipelines.



### Query and transform data

Use the Catalog SDK to query data from the Data Platform, process it, and write results back. Visualization is available at any time.

```d2
direction: right
Data Platform -> Catalog SDK: redap
Data Platform <- Catalog SDK: redap
Data Platform -> Viewer: redap
```

Minimal example of querying a dataset:

```python
import rerun as rr
client = rr.catalog.CatalogClient("rerun://example.cloud.rerun.io")
dataset = client.get_dataset("my_data")
df = dataset.reader(index="log_time")  # df is a DataFusion.DataFrame
print(df)
```

Best for: data pipelines, batch processing, ML training data preparation.

