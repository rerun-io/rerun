---
title: SDK Micro Batching
order: 6
---


The Rerun SDK automatically handles micro-batching in a background thread in order to find a sweet spot between latency and throughput, reducing metadata overhead and thus improving both bandwidth and CPU usage.

The flushing is triggered by both time and space thresholds, whichever happens to trigger first.

You can configure these thresholds using the following environment variables:

#### RERUN_FLUSH_TICK_SECS

Sets the duration of the periodic tick that triggers the time threshold, in seconds.

Defaults to `RERUN_FLUSH_TICK_SECS=0.008` (8ms).

#### RERUN_FLUSH_NUM_BYTES

Sets the size limit that triggers the space threshold, in bytes.

Defaults to `RERUN_FLUSH_NUM_BYTES=1048576` (1MiB).

#### RERUN_FLUSH_NUM_ROWS

Sets the number of rows that drives the space threshold.

Defaults to `RERUN_FLUSH_NUM_BYTES=18446744073709551615` (`u64::MAX`).
