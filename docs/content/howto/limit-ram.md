---
title: How To Limit Memory Use
order: 1
description: How to limit the memory of Rerun so that it doesn't run out of RAM.
---

### `--memory-limit`

The Rerun Viewer can not yet view more data than fits in RAM. The more data you log, the more RAM the Rerun Viewer will use. When it reaches a certain limit, the oldest data will be dropped. The default limit it to use up to 75% of the total system RAM.

You can set the limit by with the `--memory-limit` command-lint argument, or the `memory_limit` argument of [`rr.spawn`](https://ref.rerun.io/docs/python/latest/common/initialization/#rerun.spawn).

### `--drop-at-latency`

If you have multiple processes generating log data to Rerun it could happen that the Viewer builds up a backlog of unprocessed log messages. This can induce latency and also use up memory, which `--memory-limit` cannot fix. To handle this case, you can use `rerun --drop-at-latency 500ms` to start ignoring _new_ data if the input buffer exceeds 500ms of data.

This is a rarely used feature, and is mostly documented here for completeness.
