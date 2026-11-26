---
title: Stream dataset selections as Arrow batches
hidden: true
type: feature
---

### Stream dataset selections as Arrow batches

Use `DatasetSelection.read_arrow()` to stream dataset selections as PyArrow record batches and process them incrementally.
Unlike `reader()`, which returns a lazy DataFusion `DataFrame`, `read_arrow()` starts the query and returns a stream of Arrow record batches.
See the [streaming example](../query-and-transform/catalog/get-data-out.md#streaming-arrow-batches).
