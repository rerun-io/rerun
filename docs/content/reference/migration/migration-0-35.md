---
title: Migrating from 0.34 to 0.35
order: 975
---

## `--follow` has been removed

Rerun no longer supports tailing `.rrd`.
If you previously used this for live workflows, tee the data to multiple sinks instead, e.g. log to both the viewer and an `.rrd` file from the producing process.

See [the sink documentation page](../../concepts/logging-and-ingestion/sinks.md#multiple-sinks-tee-pattern) for more information on how to set up teeing.
