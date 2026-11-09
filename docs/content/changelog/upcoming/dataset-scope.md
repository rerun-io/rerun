---
title: Dataset scope for Python dataset schemas and readers
hidden: true
type: feature
---

### Dataset scope for Python dataset schemas and readers

`DatasetEntry.schema`, `DatasetEntry.reader`, `DatasetSelection.schema`, `DatasetSelection.arrow_schema` and `DatasetSelection.reader` take a `scope=` argument of type `rerun.catalog.DatasetQueryScope`.
`scope="indexed"` builds the schema from only the entity paths and timelines the dataset's chunk indexing policy indexes; `scope="full"` (the default) covers everything.

The OSS server ignores the scope.
