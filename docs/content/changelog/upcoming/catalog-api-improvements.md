---
title: Catalog API improvements
hidden: true
type: feature
---
The Python catalog API now provides `DatasetEntry.segment()` and a `Segment` handle to inspect a segment's layers and schema, get its Viewer URL, and unregister a layer.
`DatasetEntry.layer(name)` returns a live `Layer` handle whose `.segment_ids()` lists the segments currently containing that layer.
`DatasetEntry.schema()` and `DatasetView.schema()` return catalog schema snapshots, which can be converted to Arrow with `.to_arrow()`.
`DatasetEntry.arrow_schema()` is deprecated; use `DatasetEntry.schema().to_arrow()` instead.
See the [catalog object model](../concepts/query-and-transform/catalog-object-model.md#managing-segment-layers-with-the-python-catalog-api) for an example.
