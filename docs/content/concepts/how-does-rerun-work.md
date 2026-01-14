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

The viewer includes a **Chunk Store** (in-memory database for logged data) and a **gRPC endpoint** that accepts streamed data from the SDK.

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


## Common workflows

### Stream to Viewer

The simplest workflow: stream data directly from your code to the Viewer for live visualization.

```d2
direction: right
Logging SDK -> Viewer: stream
```

Best for: development, debugging, real-time monitoring.



### Save to RRD, view later

Log data to `.rrd` files, then open them in the Viewer whenever needed. Files can be loaded from disk or URLs.

```d2
direction: right
Logging SDK -> ".rrd" -> Viewer: load
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

Best for: large datasets, team collaboration, production pipelines.



### Query and transform data

Use the Catalog SDK to query data from the Data Platform, process it, and write results back. Visualization is available at any time.

```d2
direction: right
Data Platform -> Catalog SDK: redap
Data Platform <- Catalog SDK: redap
Data Platform -> Viewer: redap
```

Best for: data pipelines, batch processing, ML training data preparation.

