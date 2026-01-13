---
title: Application model
order: 0
---

The Rerun distribution comes with numerous moving pieces:

* The **SDKs** (Python, Rust & C++), for logging data and querying it back. These are libraries running directly in the end user's process.
* The **Viewer**: the Rerun GUI application for native platforms (Linux, macOS, Windows) and web. This is where logged data is visualized.
* The **Server**: a standalone service implementing the Rerun Data Protocol (redap). It provides persistent storage for datasets and serves data to Viewers or the Catalog API. Available as an open-source server (`rerun server`) or as a commercial offering.
* The **Catalog API**: a Python SDK (`rerun.catalog`) for programmatic access to data stored on a Server—query datasets, manage segments, and analyze data using DataFusion.
* The **CLI**, which allows you to control all the pieces above as well as manipulate RRD files.

The **Viewer** always includes:
  * A **Chunk Store**: an in-memory database that stores the logged data.
  * A **Renderer**: a 3D engine that renders the contents of the **Chunk Store**.
  * An integrated **gRPC endpoint** that allows SDKs to stream data directly to it.


## What runs where?

This is a lot to take in at first, but as we'll see these different pieces are generally deployed in just a few unique configurations for most common cases.

The first thing to understand is what process do each of these things run in.

The **CLI** and **Viewer** are part of the same binary: `rerun`. The CLI can also run a standalone **Server** via `rerun server`.

The **SDKs** are vanilla software libraries and therefore always execute in the same context as the end-user's code.

The **Viewer** can be executed either:
* Natively, which we call the **Native Viewer** in all our documentation, or
* On the Web as a Wasm application, which we refer to as the **Web Viewer**.

Both the Native Viewer and Web Viewer may also be extended in various ways:
* The Native Viewer may be extended through its [Rust API](../howto/visualization/extend-ui.md)
* The Web Viewer can be [embedded in web applications](../howto/integrations/embed-web.md), and used in [Jupyter Notebooks](../howto/integrations/embed-notebooks.md)

The **Web Viewer** has its own dedicated `.wasm` artifact, and always runs in isolation in the end-user's web browser.
Running the **Web Viewer** comes with [some performance limitations](#web-viewer-limitations), so you should always prefer to run the Viewer natively if it makes sense.

The **Server** runs as a standalone process, separate from the Viewer. You can run the open-source server via:
* CLI: `rerun server`
* Python: `rr.server.Server()`

The best way to make sense of it all is to look at some of the most common scenarios when logging and visualizing data.


## SDK to Viewer

The simplest workflow: one or more **SDKs**, embedded into the user's process, stream data directly to a **Viewer** via its integrated gRPC endpoint.

Logging script:

snippet: concepts/app-model/native-sync

Deployment:
<!-- TODO(#7768): talk about rr.spawn(serve=True) once that's thing -->
```sh
# Start the Rerun Native Viewer in the background.
#
# This will also start the gRPC server on its default port (9876, use `--port`
# to pick another one).
#
# We could also have just used `spawn()` instead of `connect_grpc()` in the logging
# script above, and we wouldn't have had to start the Native Viewer manually.
# `spawn()` does exactly this: it fork-execs a Native Viewer in the background
# using the first `rerun` binary available on your $PATH.
$ rerun &

# Start logging data. It will be pushed to the Native Viewer through the gRPC link.
$ ./logging_script
```


Dataflow:

```d2
direction: left

Rerun process: {
  Native Viewer: {
    Renderer
    Chunk Store
  }
  gRPC Server
}

User process: {
  SDK
}

User process.SDK -> Rerun process.gRPC Server
Rerun process.gRPC Server -> Rerun process.Native Viewer.Chunk Store
```

Reference:
* [SDK operating modes: `connect_grpc`](../reference/sdk/operating-modes.md#connect_grpc)
* [🐍 Python `connect_grpc`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.connect_grpc)
* [🦀 Rust `connect_grpc`](https://docs.rs/rerun/latest/rerun/struct.RecordingStreamBuilder.html#method.connect_grpc)
* [🌊 C++ `connect_grpc`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#aef3377ffaa2441b906d2bac94dd8fc64)


## SDK to RRD to Viewer

An asynchronous workflow: one or more **SDKs** log data to RRD files, which are later opened in the **Viewer**.

RRD files can be loaded:
* From the local filesystem
* From HTTP URLs
* Into either Native or Web Viewer

Logging script:

snippet: concepts/app-model/native-async

Deployment:
```sh
# Log the data into one or more files.
$ ./logging_script

# Start the Rerun Native Viewer and feed it the RRD file directly.
$ rerun /tmp/my_recording.rrd

# Or load from a URL
$ rerun https://example.com/recording.rrd
```

Dataflow:

```d2
direction: left

Rerun process: {
  Native Viewer: {
    Renderer
    Chunk Store
  }
  gRPC Server
}

User process: {
  SDK
}

User process.SDK -> ".rrd file"
".rrd file" -> Rerun process.Native Viewer.Chunk Store
```

Reference:
* [SDK operating modes: `save`](../reference/sdk/operating-modes.md#save)
* [🐍 Python `save`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.save)
* [🦀 Rust `save`](https://docs.rs/rerun/latest/rerun/struct.RecordingStreamBuilder.html#method.save)
* [🌊 C++ `save`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#a555a7940a076c93d951de5b139d14918)


## SDK to RRD to Server

For persistent storage and remote access, RRD files can be registered with a **Server**. This is the recommended workflow for production deployments and when working with large datasets.

### Storage model

The Server organizes data differently than the Viewer:
* **Datasets**: named collections that group related recordings
* **Segments**: individual RRD files registered to a dataset

This is different from the Viewer, which groups loose recordings by application ID.

### Local / OSS Server

For local development or self-hosted deployments, use the open-source server. It loads RRD files from the local filesystem.

Start the server with preloaded data:

```sh
# Via CLI - load all RRDs from a directory
$ rerun server --dataset my_data=/path/to/rrds/
```

Or programmatically in Python:

```python
import rerun as rr

# Start server with preloaded datasets
with rr.server.Server(datasets={"my_data": "/path/to/rrds/"}) as server:
    print(f"Server running at {server.address()}")
    # Server runs until context exits
```

### Cloud Server

For cloud deployments, RRD files are stored in object storage (S3, GCS, Azure Blob) and registered with the server via URIs.

```python
import rerun as rr

# Connect to cloud server
client = rr.catalog.CatalogClient("rerun+http://your-server:51234")

# Create or get a dataset
dataset = client.create_dataset("my_dataset")

# Register RRDs from object storage
dataset.register("s3://my-bucket/recordings/episode_001.rrd").wait()

# Or batch register multiple files
dataset.register([
    "s3://my-bucket/recordings/episode_001.rrd",
    "s3://my-bucket/recordings/episode_002.rrd",
]).wait()
```

Dataflow:

```d2
direction: left

User process: {
  SDK
}

Server process: {
  Rerun Server: {
    Catalog
    Dataset Store
  }
}

User process.SDK -> ".rrd file(s)"
".rrd file(s)" -> Server process.Rerun Server.Dataset Store
```

Reference:
* [Getting data out of Rerun](../howto/query-and-transform/get-data-out.md)


## Server to Viewer

Once data is stored on a **Server**, any **Viewer** (Native or Web) can connect to visualize it.

Connect via the Viewer UI:
1. Open the Viewer
2. Use **Add Redap server** to connect to the server address

Or via CLI:
```sh
$ rerun connect rerun+http://localhost:51234
```

Both Native and Web Viewers behave similarly when connected to a Server—they fetch and display data on demand.

Dataflow:

```d2
direction: left

Server process: {
  Rerun Server: {
    Catalog
    Dataset Store
  }
}

Viewer: {
  Renderer
  Chunk Store
}

Server process.Rerun Server.Dataset Store -> Viewer.Chunk Store: "redap (gRPC)"
```


## Server to Catalog API

The **Catalog API** provides programmatic access to data stored on a Server, without needing the Viewer. This is useful for:
* Data analysis and processing pipelines
* Building custom applications
* Automated data validation

```python
import rerun as rr

# Connect to server
client = rr.catalog.CatalogClient("rerun+http://localhost:51234")

# List available datasets
for dataset in client.datasets():
    print(f"Dataset: {dataset.name}, Segments: {len(dataset.segment_ids())}")

# Query data using DataFusion
dataset = client.get_dataset("my_dataset")
df = dataset.reader(index="log_time")

# Filter and analyze
results = df.filter(df["entity_path"] == "/camera/image").collect()
```

Dataflow:

```d2
direction: left

Server process: {
  Rerun Server: {
    Catalog
    Dataset Store
  }
}

User process: {
  Catalog API
  DataFusion
}

Server process.Rerun Server.Catalog -> User process.Catalog API: "redap (gRPC)"
User process.Catalog API -> User process.DataFusion
```

Reference:
* [Getting data out of Rerun](../howto/query-and-transform/get-data-out.md)
* [🐍 Python Catalog API](https://ref.rerun.io/docs/python/stable/common/catalog/)


## Web Viewer limitations

When running on the web as a Wasm application, the browser severely limits how much memory and compute the Viewer can use.

We currently only distribute the Viewer Wasm as 32-bit, which means it can only ever use at most 4 GiB of memory.
In practice, browsers restrict this down to around 2 GiB. When the Viewer runs out of memory, it begins to drop the oldest data in any open recordings.
This means you can't visualize larger recordings in full.

Multi-threaded Wasm is not yet generally available, and where it is available it is very inconvenient to use.
As a result, the Viewer currently runs fully single-threaded on the web. This makes it very slow compared to the native Viewer,
which can use multiple cores to ingest, process, and visualize your data.

> Note: When working with large datasets, consider using a Server. The Viewer can then fetch data on-demand rather than loading everything into memory.


## FAQ

### How can I use multiple Native Viewers at the same time (i.e. multiple windows)?

Every **Native Viewer** comes with a corresponding **gRPC endpoint** -- always. You cannot start a **Native Viewer** without starting it.

The only way to have more than one Rerun window is to have more than one gRPC endpoint, by means of the `--port` flag.

E.g.:
```sh
# starts a new viewer, listening for gRPC connections on :9876
rerun &

# does nothing, there's already a viewer session running at that address
rerun &

# does nothing, there's already a viewer session running at that address
rerun --port 9876 &

# logs the image file to the existing viewer running on :9876
rerun image.jpg

# logs the image file to the existing viewer running on :9876
rerun --port 9876 image.jpg

# starts a new viewer, listening for gRPC connections on :6789, and logs the image data to it
rerun --port 6789 image.jpg

# does nothing, there's already a viewer session running at that address
rerun --port 6789 &

# logs the image file to the existing viewer running on :6789
rerun --port 6789 image.jpg &
```

### What's the difference between the Viewer's gRPC endpoint and the Server?

The **Viewer's gRPC endpoint** is for streaming data directly into a running Viewer for immediate visualization. Data lives in the Viewer's memory.

The **Server** is for persistent storage and remote access. Data is stored in datasets (backed by files or object storage) and can be accessed by multiple Viewers or via the Catalog API.

Use the Viewer's gRPC endpoint for:
* Development and debugging
* Real-time visualization during logging

Use the Server for:
* Production deployments
* Large datasets that don't fit in memory
* Sharing data across teams
* Programmatic data access

### How do I connect a Viewer to a remote Server?

Use the `rerun+http://` URL scheme:

```sh
# Via CLI
$ rerun connect rerun+http://your-server:51234

# Or in the Viewer UI, use "Add Redap server"
```


## Appendix: Legacy web serving mode

> Note: This mode is rarely needed. For most use cases, prefer the [Server workflows](#sdk-to-rrd-to-server) described above.

The `rerun` CLI includes a legacy mode for locally testing the Web Viewer. It:
1. HTTP-serves the Wasm Viewer to a browser
2. Acts as a WebSocket proxy between the SDK and Web Viewer

This is primarily useful for development testing of the Web Viewer itself, and is being superseded by proper Server workflows.
