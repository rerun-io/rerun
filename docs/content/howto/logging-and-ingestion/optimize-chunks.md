---
title: Optimize chunk count
order: 800
---


## Understanding chunks and their impact on performance

Rerun stores all its data in Chunks — Arrow-encoded tables of data. A basic understanding of chunks is key to understanding how Rerun works and why performance behaves the way it does. See the [chunk concept documentation](../../concepts/logging-and-ingestion/chunks.md) for more details.

Chunks are the atomic unit of work in Rerun: the performance cost of logging, ingesting, storing, querying, and visualizing data (including memory overhead to some extent) scales roughly linearly with the number of chunks in the system (ignoring caching and indexing optimizations). Many small chunks cause significantly more overhead than fewer, larger ones. Larger chunks reduce index pressure and per-chunk overhead, improving write and query throughput.

Rerun provides several online and offline systems to track and reduce chunk count. Using them effectively can yield dramatic performance improvements in real-world scenarios.

The process of merging smaller chunks into fewer, larger ones is called compaction. It occurs at multiple points across the data’s lifecycle, each with different constraints. Earlier compaction reduces the work needed later in the pipeline.

Each of these stages is defined by constraints such as:
* Where it runs (e.g. client-side, server-side, standalone?)
* What data it can access (e.g. partial data or full recording? streaming data or random access possible?)
* What compute resources it can use (e.g. soft real-time or offline?)


## SDK micro-batching

Micro-batching is an online compaction mechanism on the SDK side that compacts small log calls into larger chunks before sending. A background thread flushes these batches either at fixed intervals or when they reach a size threshold.

This reduces metadata overhead (fewer chunks), which improves network and CPU efficiency. By default, the SDK flushes:
* every ~200 ms when logging to file,
* every ~8 ms when logging to the Rerun Viewer directly, or
* when the batch reaches ~1 MiB.

These defaults aim to balance latency and throughput. To adjust them, see the [micro-batching documentation](../../reference/sdk/micro-batching.md).

Micro-batching trades a bit of latency for significantly fewer chunks, improving ingestion throughput and downstream performance. While lightweight to compute, it operates with minimal context — all it sees is a small rolling window of logs — so compaction is far from optimal.

Constraints:
* Runs: client-side, in the SDK
* Data access: only a short rolling window of recent logs
* Operational limits: minimal CPU and memory usage to avoid impacting the host process


## In-viewer compaction

On the server side, the Rerun Viewer performs continuous, online compaction in the Chunk Store. As data arrives, smaller chunks are merged until they reach target sizes, preventing an explosion of tiny chunks. Triggers are based on row count and byte thresholds, similar to SDK micro-batching.

By default, chunks compact up to ~384 KiB, or ~4096 rows (or 1024 for unsorted time chunks).
These settings balance ingestion speed, query performance, and memory use. You can configure them using environment variables such as `RERUN_CHUNK_MAX_BYTES` and `RERUN_CHUNK_MAX_ROWS`. See the [store compaction docs](../../reference/store-compaction.md) for more.

Viewer-side compaction is more expensive than SDK-side micro-batching but has access to full context, enabling much more effective decisions. Fortunately, the cost is kept low thanks to micro-batching upstream: the better the batching in the SDK, the less work needed in the Viewer (as we'll see below, the CLI can even make that work disappear entirely!).

Constraints:
* Runs: server-side, in the Viewer
* Data access: the full in-memory dataset (although older data may have been [garbage collected](../visualization/limit-ram.md))
* Operational limits: must remain lightweight and responsive, as it shares CPU with other real-time viewer workloads. Runs as a streaming process — compaction happens as data arrives.


## Inspecting and compacting chunks with the Rerun CLI

Rerun offers CLI tools to inspect and optimize .rrd recordings or streamed data files.

Use [`rerun rrd stats`](../../reference/cli.md#rerun-rrd-stats) to view stats like chunk counts, sizes, and row distributions. This helps you determine if compaction is needed. For example:
```sh
$ rerun rrd stats <(curl 'https://app.rerun.io/version/latest/examples/nuscenes_dataset.rrd')

Overview
----------
num_chunks = 576
num_entity_paths = 52
num_chunks_without_components = 0 (0.000%)
num_rows = 1 563
num_rows_min = 1
num_rows_max = 101
num_rows_avg = 2.714
num_static = 46
num_indexes_min = 0
num_indexes_max = 3
num_indexes_avg = 2.760
num_components_min = 1
num_components_max = 10
num_components_avg = 1.988

Size (schema + data, uncompressed)
----------------------------------
ipc_size_bytes_total = 112 MiB
ipc_size_bytes_min = 1.4 KiB
ipc_size_bytes_max = 568 KiB
ipc_size_bytes_avg = 200 KiB
ipc_size_bytes_p50 = 161 KiB
ipc_size_bytes_p90 = 567 KiB
ipc_size_bytes_p95 = 567 KiB
ipc_size_bytes_p99 = 568 KiB
ipc_size_bytes_p999 = 568 KiB

# … truncated …
```

If a file contains many small chunks, run [`rerun rrd optimize`](../../reference/cli.md#rerun-rrd-optimize) to rewrite it with fewer, larger chunks. For example:
```sh
$ rerun rrd optimize --max-size 2MiB -o nuscenes_compacted.rrd <(curl 'https://app.rerun.io/version/latest/examples/nuscenes_dataset.rrd')
merge/compaction finished srcs=["/dev/fd/63"] time=2.51217062s num_chunks_before=576 num_chunks_after=217 num_chunks_reduction="-62.326%" srcs_size_bytes=90.0 MiB dst_size_bytes=89.6 MiB size_reduction="-0.474%"

$ rrd stats nuscenes_compacted.rrd
Overview
----------
num_chunks = 278
num_entity_paths = 52
num_chunks_without_components = 0 (0.000%)
num_rows = 1 084
num_rows_min = 1
num_rows_max = 101
num_rows_avg = 3.899
num_static = 23
num_indexes_min = 0
num_indexes_max = 3
num_indexes_avg = 2.752
num_components_min = 1
num_components_max = 10
num_components_avg = 2.133

Size (schema + data, uncompressed)
----------------------------------
ipc_size_bytes_total = 111 MiB
ipc_size_bytes_min = 1.7 KiB
ipc_size_bytes_max = 1.0 MiB
ipc_size_bytes_avg = 410 KiB
ipc_size_bytes_p50 = 567 KiB
ipc_size_bytes_p90 = 670 KiB
ipc_size_bytes_p95 = 713 KiB
ipc_size_bytes_p99 = 838 KiB
ipc_size_bytes_p999 = 1.0 MiB

# … truncated …
```

This produces a new file where chunks have been merged up to the size and row thresholds of the selected optimization profile (see below) (further capped by `--max-size 2MiB` in the example above). This significantly reduces viewer-side load and improves performance for future queries and visualization.

Because it runs offline, the CLI compactor has full access to the dataset and no real-time constraints, making it the most effective tool for optimal compaction. It's a good idea to compact files ahead of time if they’ll be queried or visualized repeatedly.

> ⚠️ `rerun rrd optimize` will automatically migrate the data to the latest version of the RRD protocol, if needed. ⚠️

Note that `rerun rrd optimize` ships two preset profiles, selected with `--profile`, that set sensible thresholds for two common targets:

* `object-store` *(default)* — large chunks (up to ~65k rows, ~2 MiB), tuned for object-store-backed datasets stored on catalog servers, where query throughput and network streaming matter most.
* `live` — small chunks (up to ~4096 rows, ~384 KiB), tuned for the live-Viewer workflow where the time panel benefits from finer-grained resolution.

Per-knob flags (`--max-rows`, `--max-size`, …) and the `RERUN_CHUNK_MAX_*` environment variables override the profile's values.

Constraints:
* Runs: standalone CLI tool
* Data access: full dataset (must fit in memory)
* Operational limits: none -- runs fully offline


## Compacting chunks with the chunk processing API

The same compaction logic that powers `rerun rrd optimize` is exposed in the [Chunk Processing API](../../concepts/logging-and-ingestion/chunk-processing-api.md), so you can fold optimization into a Python ingestion or conversion pipeline rather than running it as a separate CLI step:

snippet: howto/optimize_chunks[optimize]

[`LazyChunkStream.collect()`](https://ref.rerun.io/docs/python/stable/common/experimental?speculative-link#rerun.experimental.LazyChunkStream.collect) materializes the pipeline into a `ChunkStore`; passing an `OptimizationProfile` runs extra compaction passes tuned for a specific target. The two presets mirror the CLI's `--profile` values:

* `OptimizationProfile.OBJECT_STORE` (corresponds to `--profile object-store`, the CLI default) — large chunks for object-store-backed datasets;
* `OptimizationProfile.LIVE` (corresponds to `--profile live`) — small chunks for the live-Viewer workflow.

* **Note:** `collect()` materializes the entire pipeline into an in-memory `ChunkStore` before writing, so the full recording must fit in RAM.


## Conclusion

* Compaction isn’t a minor optimization — it can and frequently yields massive performance gains depending on your workload.
* Rerun applies micro-batching and compaction by default, but optimal settings vary per use case.
* Compaction can (and should) happen at multiple stages, each with different tradeoffs, operating under very different constraints.
* Once data has been recorded, two complementary tools let you preemptively optimize it for downstream use:
    * The Rerun CLI: `rerun rrd stats` to diagnose, `rerun rrd optimize` for one-shot offline compaction.
    * The [Chunk Processing API](../../concepts/logging-and-ingestion/chunk-processing-api.md): same compaction logic, exposed in-process so you can fold it into a Python ingestion or conversion pipeline via `collect(optimize=OptimizationProfile.…)`.
