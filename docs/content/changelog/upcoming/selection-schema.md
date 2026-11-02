---
title: "`DatasetSelection.schema()` covers only the selected segments"
hidden: true
type: breaking
---

The schema of a selection with a segment filter now holds only the columns of the selected segments.
Previously, it held the columns of every segment of the dataset.
A segment filter requires a server running Rerun 0.41 or Rerun Hub 0.20 or later, and older servers raise an error.

#### Migration

To get the schema of the whole dataset, as before, ask the dataset instead of the selection:

```python
selection = dataset.filter_segments(["first", "second"])

# Before: the columns of every segment. Now: the columns of `first` and `second`.
schema = selection.schema()

# The columns of every segment, before and now.
schema = selection.dataset.schema()
```

`selection.dataset.schema()` ignores content filters: apply them to the dataset, as in `dataset.filter_contents("/robot/**").schema()`.
