---
title: "`rerun.catalog.TimelineKind`"
hidden: true
type: misc
---

`TimelineInfo.kind` is now a `rerun.catalog.TimelineKind` enum, which maps to and from the Arrow datatype of the timeline's index column with `kind.datatype` and `TimelineKind.from_datatype(…)`.
It is a string enum, so comparisons such as `kind == "sequence"` keep working.
