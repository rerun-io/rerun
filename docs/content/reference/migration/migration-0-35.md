---
title: Migrating from 0.34 to 0.35
order: 975
---

## `StateChange.state` is now an array

The `state` field of the [`StateChange`](https://rerun.io/docs/reference/types/archetypes/state_change) archetype now takes an array of values instead of a single value.
Each entry gets its own lane in the state timeline view, so one entity can track several states at once (e.g. the buttons of a gamepad).

Nothing changes on the wire or in stored recordings — this only affects the SDK APIs.

### Rust

`with_state` now takes an iterator of values, so passing a single string no longer compiles.
Use the new `StateChange::single` convenience constructor, or pass an array:

```rust
// 0.34
rec.log("door", &rerun::StateChange::new().with_state("open"))?;

// 0.35
rec.log("door", &rerun::StateChange::single("open"))?;
// or, equivalently:
rec.log("door", &rerun::StateChange::new().with_state(["open"]))?;
```

To reset the state of individual instances, use the new `with_state_opt`, where a `None` entry resets that instance's lane:

```rust
rec.log("buttons", &rerun::StateChange::new().with_state_opt([Some("Idle"), None]))?;
```

### Python

No action needed.
`rr.StateChange(state="open")` keeps working, and `state=["idle", "pressed"]` is now supported for multiple lanes.

### C++

No action needed.
`rerun::StateChange().with_state("open")` keeps working, and `with_state({"idle", "pressed"})` is now supported for multiple lanes.

## `ParquetReader` index columns now use `IndexColumn`

The experimental `ParquetReader`'s `index_columns` argument no longer takes `(name, type[, unit])` tuples.
Pass [`IndexColumn`](https://rerun.io/docs/reference/python/latest/rerun/experimental#rerun.experimental.IndexColumn) values instead, built with the `timestamp`/`duration`/`sequence` constructors (the timeline kind is the constructor you pick, and `unit` is now the keyword-only `input_unit`):

```python
# 0.34
ParquetReader(path, index_columns=[("frame", "sequence"), ("ts", "timestamp", "ms")])

# 0.35
from rerun.experimental import IndexColumn

ParquetReader(path, index_columns=[IndexColumn.sequence("frame"), IndexColumn.timestamp("ts", input_unit="ms")])
```

## `--follow` has been removed

Rerun no longer supports tailing `.rrd`.
If you previously used this for live workflows, tee the data to multiple sinks instead, e.g. log to both the viewer and an `.rrd` file from the producing process.

See [the sink documentation page](../../concepts/logging-and-ingestion/sinks.md#multiple-sinks-tee-pattern) for more information on how to set up teeing.
