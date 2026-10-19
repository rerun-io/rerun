---
title: "`DatasetView` renamed to `DatasetSelection`"
hidden: true
type: breaking
---

### `DatasetView` renamed to `DatasetSelection`

`rerun.catalog.DatasetView`, returned by `filter_segments()` and `filter_contents()`, is now `rerun.catalog.DatasetSelection`, with unchanged behavior.
The old name still works but emits a `DeprecationWarning`; use the new name in type hints and `isinstance` checks.
