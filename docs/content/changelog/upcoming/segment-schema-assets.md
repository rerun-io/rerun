---
title: "`Segment.schema()` can include assets"
hidden: true
type: misc
---

`Segment.schema(include_assets=True)` adds the columns of the assets that apply to the segment, matching the schema of `DatasetEntry.segment_store(…, include_assets=True)`.
The default, `include_assets=False`, returns the segment's own layers only, as before.
