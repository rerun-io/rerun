---
title: Dataset scope for Python queries
hidden: true
type: feature
---

### Dataset scope for Python queries

`DatasetEntry.reader` and `DatasetSelection.reader` take a `scope=` argument of type `rerun.catalog.DatasetQueryScope`, which selects the data a query reads relative to the dataset's chunk indexing policy.
On Rerun Hub, `scope="indexed"` reads only indexed data: selecting every entity path selects only the indexed ones, and naming an entity path or timeline the policy excludes raises `rerun.catalog.ChunkNotIndexedError`.
`scope="full"`, the default, covers everything.
On Rerun Hub, it reads data outside the policy from the recordings themselves.
That works only for specific segments, so filter the dataset to them first.
Without them, such a read raises `rerun.catalog.ChunkNotIndexedError`.

`DatasetEntry.schema`, `DatasetSelection.schema` and `DatasetSelection.arrow_schema` also take `scope=`.
The OSS server ignores the scope.
