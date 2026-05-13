---
title: Chunk Processing API
order: 750
---

The Chunk Processing API is a flexible, [chunk](chunks.md)-centric API for data ingestion, transformation, and conversion pipelines.
It covers I/O from common robotics file formats, powerful declarative data wrangling primitives, and a multithreaded, native engine for pipeline execution.
The API is designed to support distributed execution in the future.

> [!NOTE]
> The Chunk Processing API is currently experimental and may change in future releases. It is available in the Python SDK under `rerun.experimental`.

## Building blocks

The Chunk Processing API is built from three kinds of primitives — readers, stores, and lazy streams — that compose into a pipeline executed by a terminal call:

```d2
direction: right

Reader: {
  shape: parallelogram
}

Store: {
  shape: cylinder
}

LazyChunkStream: {
  shape: rectangle
  label: |md `LazyChunkStream`|
}

Terminal call: {
  shape: parallelogram
  label: "Terminal\ncall"
}


Reader -> Store: |md `.store()`|
Reader -> LazyChunkStream: |md `.stream()`|
Store -> LazyChunkStream: |md `.stream()`|
LazyChunkStream -> LazyChunkStream: |md `filter`/`lenses`/`map`/…|
LazyChunkStream -> Terminal call
```

### Readers

Readers produce [`Chunk`](chunks.md)s from external sources such as files, or datasets hosted on a catalog server.

In some cases, readers are classes provided by the Chunk Processing API, such as [`RrdReader`](https://ref.rerun.io/docs/python/stable/common/experimental?speculative-link#rerun.experimental.RrdReader) and [`McapReader`](https://ref.rerun.io/docs/python/stable/common/experimental?speculative-link#rerun.experimental.McapReader).
The reader functionality can also be provided by classes from other parts of the Rerun SDK.
For example, [`DatasetEntry`](https://ref.rerun.io/docs/python/stable/common/catalog?speculative-link#rerun.catalog.DatasetEntry) has a [`segment_store`](https://ref.rerun.io/docs/python/stable/common/catalog?speculative-link#rerun.catalog.DatasetEntry.segment_store) method which returns a [`LazyStore`](https://ref.rerun.io/docs/python/stable/common/experimental?speculative-link#rerun.experimental.LazyStore) for the corresponding segment (see the [catalog object model](../query-and-transform/catalog-object-model.md) for more information on datasets).
[`UrdfTree`](https://ref.rerun.io/docs/python/stable/common/urdf?speculative-link#rerun.urdf.UrdfTree) is another example of a class that offers reader functionality in addition to a larger feature set.

There are two ways in which a reader may provide chunks.
All readers can sequentially stream all their source's chunks, typically via the `stream()` method.
Internally, such readers typically parse the source file, convert data to chunks as it is extracted, and yield those chunks as they are produced.

Some readers, called [`IndexedReader`](https://ref.rerun.io/docs/python/stable/common/experimental?speculative-link#rerun.experimental.IndexedReader), can also provide indexed, random access to chunks via a [`LazyStore`](https://ref.rerun.io/docs/python/stable/common/experimental?speculative-link#rerun.experimental.LazyStore).
This is typically implemented on top of an existing chunk index, and is currently available for the following readers:
- `RrdReader` (relies on the RRD footer index) <!-- TODO(ab) link doc page about that when we have it -->
- `DatasetEntry.segment_store()` (relies on the chunk index maintained by the catalog server)

Processing chunks through a `LazyStore` is beneficial for pipelines where only a subset of chunks is needed, avoiding the I/O cost of loading unnecessary ones.

> [!NOTE]
> Filter pushdown to `LazyStore` (e.g. `lazy_store.stream().filter(content="/my/entity")`) is planned but not yet implemented; today the filter runs after the chunks have been loaded.

In all cases, readers typically act as the root of a processing pipeline and provide a `LazyChunkStream` object to refine and execute it — see [Lazy stream](#lazy-stream) below.


### Stores

A store is a collection of chunks and comes in two complementary flavors:

- **[`LazyStore`](https://ref.rerun.io/docs/python/stable/common/experimental?speculative-link#rerun.experimental.LazyStore)** — index-based, on-demand. Returned by indexed loaders such as `RrdReader(path).store()` and `DatasetEntry.segment_store()`.
- **[`ChunkStore`](https://ref.rerun.io/docs/python/stable/common/experimental?speculative-link#rerun.experimental.ChunkStore)** — fully materialized, all chunks held in memory. Build one with `ChunkStore.from_chunks([...])`, or materialize a stream via `stream.collect()`.


The previous section already hinted at the perks of `LazyStore`. Being index-based, it is cheap to create and takes limited amounts of memory.
Also, it unlocks performance speed-ups by only loading chunks that are relevant to the given processing pipeline.
On the other hand, `ChunkStore` is fully materialized: its memory footprint scales with the recording size.
This is a major exception in the chunk processing API, which generally leans on lazy loading and streaming execution to allow processing large datasets with bounded memory.

One common reason to materialize a `ChunkStore` is to run chunk optimization; see [Optimize chunk count](../../howto/logging-and-ingestion/optimize-chunks.md#compacting-chunks-with-the-chunk-processing-api) for details.

> [!NOTE]
> In the future, `ChunkStore` will be extended to allow running [dataframe queries](../query-and-transform/dataframe-queries.md) directly against it.

Both kinds of stores share a common API surface, including:
- extracting the underlying [`Schema`](https://ref.rerun.io/docs/python/stable/common/catalog?speculative-link#rerun.catalog.Schema) of the store;
- turning the store back into a pipeline with `.stream()`;
- exposing various statistics and content summaries.


### Lazy stream

The `LazyChunkStream` is the central abstraction: a deferred, single-pass iterator of chunks with operators for filtering (`filter` / `drop`), branching (`split`), fan-in (`merge`), reshaping (`lenses`), and arbitrary per-chunk manipulation (`map` / `flat_map`).

The key design is that a lazy stream is not a materialized collection or actual streaming process.
A `LazyChunkStream` instance can be thought of as a leaf node in a pipeline-description [DAG](https://en.wikipedia.org/wiki/Directed_acyclic_graph).
By composition, it allows building up the DAG to represent the intended pipeline.

For example, this creates a basic pipeline that does nothing but read an MCAP file:

snippet: concepts/chunk_processing_intro[read]

This pipeline can be extended using the lazy stream's methods.
For example, we can add a filter operation:

snippet: concepts/chunk_processing_intro[filter]

Up to this point, no data has actually been read or processed.
This happens when a terminal operation is called, for example:

snippet: concepts/chunk_processing_intro[terminal]

This exact call triggers the pipeline execution, including reading the source MCAP, performing the filter operation, and writing the output RRD.

#### Pipeline execution

To recap:

- A pipeline is a DAG rooted at one or more readers or stores and ending at a leaf node represented by a lazy stream.
- Composition is cheap: building the DAG is metadata only, regardless of input size. This is done through `LazyChunkStream`'s APIs.
- The actual execution of the pipeline is triggered by calling a terminal method of the lazy stream, for example `.write_rrd()`. Terminal calls are blocking, but execution is multithreaded and essentially GIL-free.
- Memory cost is bounded by what flows through a chunk at a time, not by the total recording size.

#### Move semantics

To better express the DAG composition process, `LazyChunkStream` instances exhibit Rust-like move semantics to avoid accidental reuse:

- `stream.filter(...)` moves `stream` into the new pipeline. Reusing `stream` afterwards raises `ValueError: already been consumed`.
- `stream.split(...)` returns two branches and consumes the parent. Each branch is itself a stream that can only be consumed once.
- `LazyChunkStream.merge(a, b, ...)` consumes every input.

Terminal calls, however, do not consume the stream — a lazy stream can be executed multiple times against different destinations:

```python
chunk_list = stream.to_chunks()
stream.write_rrd(path=..., application_id=..., recording_id=...)
```

Note that doing so executes the entire pipeline twice, which may not be desirable for complex pipelines. In that case, collect the stream to an intermediate `ChunkStore` to trade memory for re-computation.

## Complete example

The rest of this page walks through a single end-to-end pipeline that reads a robot-arm MCAP recording, fans the protobuf joint-state column out into per-joint `Scalars` series in degrees, tags the result with a static `/metadata` chunk built from scratch, and writes a new `.rrd`.

Full source: [Python](https://github.com/rerun-io/rerun/blob/main/docs/snippets/all/concepts/chunk_processing.py?speculative-link).

### Setup

snippet: concepts/chunk_processing[setup]

- Imports the experimental entry points: readers (`McapReader`), chunk and stream types (`Chunk`, `LazyChunkStream`), lens primitives (`DeriveLens`, `Selector`).
- Locates the input MCAP relative to the repo root and picks a CWD-relative output path. Nothing here touches Rerun yet.

### Reading

snippet: concepts/chunk_processing[reading]

- `McapReader(MCAP).stream()` is the only line that touches the source — and even that is lazy: no MCAP bytes are decoded yet.
- The returned `LazyChunkStream` is the root of the DAG.

### Processing

snippet: concepts/chunk_processing[processing]

- `drop(content="/video_raw/**")` is a no-op against this MCAP (the path does not exist) but illustrates content-based pruning.
- `fan(side)` builds six `DeriveLens` instances, one per joint, each extracting `.joint_positions[i]` (via `Selector(...).pipe(...)`), converting radians to degrees with `pyarrow.compute`, and routing the result to `/joints_deg/<side>/<joint>` as a `Scalars` column.
- Two scoped `.lenses(...)` calls apply the per-side fan only to chunks under `/robot_left/**` and `/robot_right/**` respectively. The same component name (`schemas.proto.JointState:message`) lives on both sides; scoping by `content=` is what disambiguates them. With `forward_unmatched`, every chunk outside the scope passes through untouched.

### Merging

snippet: concepts/chunk_processing[merging]

- `Chunk.from_columns("/metadata", indexes=[], columns=rr.AnyValues.columns(…))` builds a single static chunk from scratch — `indexes=[]` makes it static. Any archetype's `.columns(…)` helper works here.
- `LazyChunkStream.from_iter([metadata])` lifts that one chunk into a one-element stream so it can participate in the pipeline.
- `LazyChunkStream.merge(processed, ...)` is fan-in: the two inputs become one stream. Order is preserved per-input, not globally.

### Writing

snippet: concepts/chunk_processing[write]

- `write_rrd(...)` is the terminal: this is where the DAG actually executes. The whole pipeline runs in a single streaming pass.
- `application_id` and `recording_id` identify the resulting recording; a fresh `uuid.uuid4()` makes each invocation produce a distinct recording.

## Relationship to the logging APIs

Both the logging APIs (`rr.log`, `rr.send_columns`, `RecordingStream`) and the Chunk Processing API target the same underlying data model, but they differ in several ways:

|                        | Logging API                              | Chunk processing API                                                                                    |
|------------------------|------------------------------------------|---------------------------------------------------------------------------------------------------------|
| Direction              | logging call → sink                      | chunk source → transform → chunk sink                                                                   |
| Granularity            | single rows or columns of data           | whole chunks                                                                                            |
| Execution model        | continuous, as logging calls are emitted | lazy, upon stream execution                                                                             |
| Where chunks come from | built by the logging API's batcher       | already exist (from a reader) or built explicitly with `Chunk.from_columns` / `Chunk.from_record_batch` |
| Typical use            | realtime data logging                    | ingestion, conversion, post-processing pipelines                                                        |

The two are interoperable:
- **Logging → chunk processing:** save a `RecordingStream` to an `.rrd`, then re-open it with `RrdReader` to get a `LazyChunkStream`.

  > [!NOTE]
  > This roundtrip-via-file will be smoothed out in the future for better ergonomics and performance.
- **Chunk processing → logging:** `rerun.experimental.send_chunks(chunks, recording=...)` feeds chunks into an active `RecordingStream` (useful for streaming to a viewer, for example).
- **Building chunks by hand:** `Chunk.from_columns` mirrors `rr.send_columns` and accepts the same `rr.<Archetype>.columns(...)` helpers, so any data that can be logged with `rr.send_columns` can also be packaged as a `Chunk` and injected into a processing pipeline.


## See also

- [Chunks](chunks.md): the underlying data model.
- [Lenses](../query-and-transform/lenses.md): the reshaping primitives used here.
