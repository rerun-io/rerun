---
title: Limit the viewer's memory usage
order: 0
description: How to limit the memory used by the Rerun Viewer so that it doesn't run out of RAM.
---

### --memory-limit

The Rerun Viewer can not yet view more data than fits in RAM. The more data you log, the more RAM the Rerun Viewer will use. When it reaches a certain limit, the oldest data will be dropped. The default limit it to use up to 75% of the total system RAM.

You can set the limit by with the `--memory-limit` command-lint argument, or the `memory_limit` argument of [`rr.spawn`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.spawn).
