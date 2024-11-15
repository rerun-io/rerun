---
title: Datastore compaction
order: 900
---


The Rerun datastore continuously compacts data as it comes in, in order find a sweet spot between ingestion speed, query performance and memory overhead.

The compaction is triggered by both number of rows and number of bytes thresholds, whichever happens to trigger first.

This is very similar to, and has many parallels with, the [micro-batching mechanism running on the SDK side](./sdk/micro-batching.md).

You can configure these thresholds using the following environment variables:

#### RERUN_CHUNK_MAX_BYTES

Sets the threshold, in bytes, after which a `Chunk` cannot be compacted any further.

Defaults to `RERUN_CHUNK_MAX_BYTES=4194304` (4MiB).

#### RERUN_CHUNK_MAX_ROWS

Sets the threshold, in rows, after which a `Chunk` cannot be compacted any further.

Defaults to `RERUN_CHUNK_MAX_ROWS=4096`.

#### RERUN_CHUNK_MAX_ROWS_IF_UNSORTED

Sets the threshold, in rows, after which a `Chunk` cannot be compacted any further.
Applies specifically to _non_ time-sorted chunks, which can be slower to query.

Defaults to `RERUN_CHUNK_MAX_ROWS=1024`.
