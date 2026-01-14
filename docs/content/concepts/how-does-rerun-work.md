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

Both versions include a **Chunk Store** (in-memory database for logged data) and a **gRPC endpoint** that accepts streamed data from the SDK.

The Web Viewer has performance limitations compared to the native viewer. It runs as 32-bit Wasm and is limited to ~2 GiB memory in practice, limiting the amount of data that can be visualized simultaneously. It also runs single-threaded, making it generally slower than native.

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
".rrd files" -> Data Platform.Datasets: register
Data Platform.Datasets -> Viewer.Chunk Store: redap
Data Platform.Datasets -> Catalog SDK: redap
```


## What ships where

| Artifact          | Includes                             |
|-------------------|--------------------------------------|
| **CLI** (`rerun`) | Viewer, OSS Data Platform, RRD tools |
| **Python SDK**    | Logging SDK, Catalog SDK, CLI        |
| **Rust SDK**      | Logging SDK                          |
| **C++ SDK**       | Logging SDK                          |
| **Web Viewer**    | Wasm artifact (hosted separately)    |


## Common patterns

- **Stream data live** → [Logging and ingestion](logging-and-ingestion.md)
- **Save and load recordings** → [Logging and ingestion](logging-and-ingestion.md)
- **Query data programmatically** → [Query and transform](query-and-transform.md)
- **Visualize from Data Platform** → [Query and transform](query-and-transform.md)
