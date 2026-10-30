---
title: "`Component.table_field()`"
hidden: true
type: misc
---

`rr.components.Flag.table_field("reviewed", editable=True)` returns a catalog table schema field with a Rerun component type, which gives the column semantic meaning.
The Viewer uses it to pick the UI for the column.
With `editable=True`, the column can be edited without a table blueprint.

```python
schema = pa.schema([
    pa.field("id", pa.int64(), metadata={rr.SORBET_IS_TABLE_INDEX: "true"}),
    rr.components.Url.table_field("link"),
    rr.components.Flag.table_field("reviewed", editable=True),
    rr.components.Text.table_field("notes", editable=True),
])
```

See [Give columns semantic meaning](../howto/query-and-transform/catalog-tables.md#give-columns-semantic-meaning).
