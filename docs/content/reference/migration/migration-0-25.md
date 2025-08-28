---
title: Migrating from 0.24 to 0.25
order: 985
---
<!--   ^^^ this number must be _decremented_ when you copy/paste this file -->


## Removed deprecated `--serve` CLI argument

Use `--web-viewer` instead.


## Removed the `--drop-at-latency` CLI argument

This feature has been defunct for a while. A better replacement can be tracked [in this issue](https://github.com/rerun-io/rerun/issues/11024).


## Flush takes an optional timeout, and returns errors
When flushing a recording stream you can now give it a maximum time for how long it should block.
The flush will block until either it completes, fails (e.g. because of connection loss), or the timeout is reached.

Previously this could only be configured for gRPC sinks, and it was configured once then first connecting.

In the C and Python APIs, negative timeouts used to have special meaning. Now they are no longer permitted.

The Python flush calls now raises an error if the flushing did not complete successfully.


## Changed arrow encoding of blobs
We used to encode blobs as `List<uint8>`, which was rather unidiomatic.
Now they are instead encoded as `Binary`.
Old data will be migrated on ingestion (zero-copy).

Affects the following components:
- [`Blob`](https://rerun.io/docs/reference/types/components/blob)
- [`ImageBuffer`](https://rerun.io/docs/reference/types/components/image_buffer)
- [`VideoSample`](https://rerun.io/docs/reference/types/components/video_sample)
