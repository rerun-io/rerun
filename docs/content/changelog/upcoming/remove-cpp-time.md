---
title: Removed deprecated C++ time setters and column constructors
hidden: true
type: breaking
---

Rerun 0.44 removes the following `RecordingStream` setters and `TimeColumn` constructors, deprecated in 0.23.

To preserve the old duration timeline, replace `rec.set_time(name, point)` with `rec.set_time_duration(name, point.time_since_epoch())` for a time point, or `rec.set_time_duration(name, duration)` for a duration.
Use `set_time_timestamp` only if you intend to change the timeline to a timestamp.
Replace `set_time_seconds` and `set_time_nanos` with `set_time_duration_secs` and `set_time_duration_nanos`.
For `TimeColumn`, replace `from_sequence_points` with `from_sequence` and `from_times` with `from_durations`.
Replace `from_seconds` and `from_nanoseconds` with `from_duration_secs` and `from_duration_nanoseconds` to keep their duration semantics.
