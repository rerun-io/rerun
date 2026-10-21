---
title: Catalog API improvements
hidden: true
type: feature
---

### Catalog API improvements

- `DatasetEntry.segment()` returns a `Segment` handle to inspect a segment's layers and schema, get its Viewer URL, and unregister a single layer.
  See [managing segment layers](../concepts/query-and-transform/catalog-object-model.md#managing-segment-layers-with-the-python-catalog-api).
- `DatasetEntry.layer(name)` returns a live `Layer` handle whose `.segment_ids()` lists the segments currently containing that layer.
- `DatasetEntry.schema()` and `DatasetSelection.schema()` return catalog schema snapshots, which convert to Arrow with `.to_arrow()`.
  `DatasetEntry.arrow_schema()` is deprecated in favor of `DatasetEntry.schema().to_arrow()`.
- `DatasetEntry.info()` summarizes a dataset (segment count, size, layers, timeline ranges, contents and property columns) without reading any recording data.
  See [getting an overview](../howto/query-and-transform/get-data-out.md#getting-an-overview).
