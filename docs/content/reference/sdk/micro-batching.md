---
title: Micro Batching
order: 700
---


The Rerun SDK automatically handles micro-batching in a background thread in order to find a sweet spot between latency and throughput, reducing metadata overhead and thus improving both bandwidth and CPU usage.

The flushing is triggered by both time and space thresholds, whichever happens to trigger first.

This is very similar to, and has many parallels with, the [compaction mechanism running on the datastore side](../store-compaction.md).

You can configure these thresholds using the following environment variables:

#### RERUN_FLUSH_TICK_SECS

Sets the duration of the periodic tick that triggers the time threshold, in seconds.

Defaults to `RERUN_FLUSH_TICK_SECS=0.2` (200ms), unless the recording stream uses a networking sink, which defaults to `RERUN_FLUSH_TICK_SECS=0.008` (8ms).

#### RERUN_FLUSH_NUM_BYTES

Sets the size limit that triggers the space threshold, in bytes.

Defaults to `RERUN_FLUSH_NUM_BYTES=2097152` (2MiB).

#### RERUN_FLUSH_NUM_ROWS

Sets the number of rows that drives the space threshold.

Defaults to `RERUN_FLUSH_NUM_ROWS=18446744073709551615` (`u64::MAX`).

#### RERUN_CHUNK_MAX_ROWS_IF_UNSORTED

Splits a chunk if it has at least this many rows and at least one of its timelines is unsorted.

Defaults to `RERUN_CHUNK_MAX_ROWS_IF_UNSORTED=8192`.

Or directly from code, in Python & Rust:

snippet: howto/micro_batching

## Backpressure

The batcher limits how much data can wait in its queues.
The limit is 100 MiB by default, split between the queue into the batcher and the queue out of it.
When a queue is full, log calls block until the sink catches up.
This happens when you log faster than the sink can write, for example over a slow network or before a gRPC connection is made.
A single chunk larger than the limit is still processed.

The limit is the `max_bytes_in_flight` field of `ChunkBatcherConfig`.
It can only be changed from Rust, and only when the recording stream is created.
