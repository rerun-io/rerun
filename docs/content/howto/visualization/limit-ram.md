---
title: Limit the viewer's memory usage
order: 0
description: Keep the Viewer from running out of memory
---

### --memory-limit

The Rerun Viewer can not yet view more data than fits in RAM. The more data you log, the more RAM the Rerun Viewer will use. When it reaches a certain limit, the oldest data will be dropped. The default limit is to use up to 75% of the total system RAM.

You can set the limit with the `--memory-limit` command-line argument, or the `memory_limit` argument of [`rr.spawn`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.spawn).

Alternatively, you can adjust the limit in the Viewer's settings.
This setting is remembered between sessions, but `--memory-limit` overrides it, and `rr.spawn` always passes that flag (defaulting to `75%`).

To find out what is using up the memory, see [Diagnose latency and performance](diagnose-performance.md).
