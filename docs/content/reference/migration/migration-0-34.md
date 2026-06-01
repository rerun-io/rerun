---
title: Migrating from 0.33 to 0.34
order: 976
---

## `rerun.recording` module removed

The `rerun.recording` module — `Recording`, `RRDArchive`, `load_recording`, `load_archive` — has been removed, having been deprecated in 0.32.
The related `rr.send_recording()`, `RecordingStream.send_recording()`, `Recording.from_chunks()`, and `DatasetEntry.download_segment()` are removed as well.

Use `rerun.experimental.RrdReader` instead.
See the [0.32 migration guide](migration-0-32.md#rerunrecording-deprecated-in-favor-of-rrdreader) for more details.
