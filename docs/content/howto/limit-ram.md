---
title: How To Limit Memory Use
order: 1
description: How to limit the memory of Rerun so that it doesn't run out of RAM. 
---

### `--memory-limit`

The Rerun Viewer can not yet view more data than fits in RAM. The more data you log, the more RAM the Rerun Viewer will use. The RAM use will build up until you run out of memory. This can be fixed by starting the viewer from the command-line with the `--memory-limit` argument.

For instance, if you run `rerun --memory-limit 16GB` then the viewer will start throwing away the oldest logged so as not to go over that 16 GB limit.

NOTE: This currently only work when you are using [`rr.connect`](https://ref.rerun.io/docs/python/latest/common/initialization/#rerun.connect) to connect to a separate `rerun` process. There is currently no way of specifying a memory limit when using `rr.spawn`.

### `--drop-at-latency`

If you have multiple processes generating log data to Rerun it could happen that the Viewer builds up a backlog of unprocessed log messages. This can induce latency and also use up memory, which `--memory-limit` cannot fix. To handle this case, you can use `rerun --drop-at-latency 500ms` to start ignoring _new_ data if the input buffer exceeds 500ms of data.

This is a rarely used feature, and is mostly documented here for completeness.
