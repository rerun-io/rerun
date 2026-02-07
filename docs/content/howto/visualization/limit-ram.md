---
title: Limit the viewer's memory usage
order: 0
description: How to limit the memory used by the Rerun Viewer so that it doesn't run out of RAM.
---

### --memory-limit

The Rerun Viewer can not yet view more data than fits in RAM. The more data you log, the more RAM the Rerun Viewer will use. When it reaches a certain limit, the oldest data will be dropped. The default limit is to use up to 75% of the total system RAM.

You can set the limit with the `--memory-limit` command-line argument, or the `memory_limit` argument of [`rr.spawn`](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.spawn).

Alternatively, you can adjust the limit for an active session also in the viewer's settings. It will be reset to the default the next time you open the viewer.
