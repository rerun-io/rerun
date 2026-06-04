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
