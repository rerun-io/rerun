---
title: Migrating from 0.33 to 0.34
order: 976
---

## `log_tick` no longer logged by default; `log_time` can be disabled

The SDK no longer injects the `log_tick` timeline column into logged data by default.
The `log_time` timeline is still injected by default, but can now be disabled.

The initial defaults are controlled by environment variables, read once on first use:

| Variable         | Default | Effect                                                       |
|------------------|---------|--------------------------------------------------------------|
| `RERUN_LOG_TICK` | off     | Set truthy (`1`/`true`/`on`/…) to inject the `log_tick` timeline. |
| `RERUN_LOG_TIME` | on      | Set falsy (`0`/`false`/`off`/…) to skip the `log_time` timeline.  |

They can also be toggled at runtime, either on the active recording or on a specific `RecordingStream`:

snippet: migration/log_tick_enabled

If you relied on the `log_tick` timeline being present, set `RERUN_LOG_TICK=1` (or call `set_log_tick_enabled(true)`) to restore the old behavior.

## `rerun.recording` module removed

The `rerun.recording` module — `Recording`, `RRDArchive`, `load_recording`, `load_archive` — has been removed, having been deprecated in 0.32.
The related `rr.send_recording()`, `RecordingStream.send_recording()`, `Recording.from_chunks()`, and `DatasetEntry.download_segment()` are removed as well.

Use `rerun.experimental.RrdReader` instead.
See the [0.32 migration guide](migration-0-32.md#rerunrecording-deprecated-in-favor-of-rrdreader) for more details.

## Remove embedded base64-encoded table blueprints & replace with blueprint registration

Table blueprints are no longer read from the Arrow schema metadata key `rerun:table_blueprint`.
If you previously stored `base64:…` encoded `.rbl` bytes in table metadata, export that blueprint as a regular `.rbl` file and register it with `TableEntry.register_blueprint(...)` instead.
Tables without a registered blueprint fall back to Arrow field metadata and viewer heuristics.

> [!NOTE]
> As of this release table blueprints alongside dataset preview are still regarded as an
> experimental feature which means that the table & APIs for table blueprints may change significantly.

## `DatasetEntry.manifest()` deprecated

`DatasetEntry.manifest()` was always intended for internal and debugging use only and should never have been part of the public API.
It is now marked `@deprecated` and will be removed in a future release.
No public replacement is offered.

## Remove previously deprecated SDK methods for custom indices

The `DatasetEntry` methods `create_fts_search_index`,  `create_vector_search_index`, `delete_search_indexes`, `search_fts`, and `search_vector` have been removed, having been deprecated in 0.31.

This change does not impact your ability to search through your dataset via [dataframe queries](https://rerun.io/docs/concepts/query-and-transform/dataframe-queries).

