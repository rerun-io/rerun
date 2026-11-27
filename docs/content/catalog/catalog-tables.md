---
title: Catalog tables
order: 600
description: Persist tabular data such as metrics, annotations, and query results on a catalog server
---

Catalog tables let you persist arbitrary tabular data, including results derived from queries on your datasets.
Examples include per-segment metrics, evaluation results, annotations, and processing status.
A common use case is to save these results so you can share them with colleagues, then inspect or update them later.
Use [properties in a layer](layers.md#adding-properties-to-segments-using-layers) instead for values that belong to a specific segment and should appear as columns of its segment table.

Catalog tables use Arrow schemas, and catalog query APIs return lazy DataFusion dataframes.
Unlike datasets, tables use _schema-on-write_: you define the table's Arrow schema before writing rows.
See the [catalog object model](catalog-object-model.md#table-entries) for how tables relate to other catalog entries.

If you only want to display an ephemeral table in a running Viewer, see [Send tables to Rerun](../log/send-data/send-table.md) instead.

[Storing query results in a catalog table](store-query-results.md) walks through a complete example: it creates a table from a dataset query, reads it back, joins it with the dataset, and updates it.

## Create a table

[`CatalogClient.create_table`](https://ref.rerun.io/docs/python/stable/catalog/#rerun.catalog.CatalogClient.create_table) creates an empty table from an Arrow schema and returns its [`TableEntry`](https://ref.rerun.io/docs/python/stable/catalog/#rerun.catalog.TableEntry).
In a later session, retrieve the table by name with `get_table`.

On a local catalog server, you can pass `url` to `create_table` to choose the storage directory.
If you omit it, the table goes in a temporary directory that the server deletes when it stops.
Rerun Hub always uses its configured storage instead, so do not pass `url`.

## Read a table

Call `reader()` on the table entry to get a DataFusion dataframe that you can filter, aggregate, or join.
To join a table with the segments of a dataset, see [Join with a dataset](store-query-results.md#join-with-a-dataset).

## Update a table

[`DataFrame.write_table`](https://datafusion.apache.org/python/autoapi/datafusion/dataframe/index.html#datafusion.dataframe.DataFrame.write_table) executes a lazy DataFusion query and appends its rows to a table without first collecting the result into Python.
For direct updates, a `TableEntry` supports three mutation operations using Arrow record batches:

- `append` adds rows to the existing table.
- `overwrite` replaces all existing rows.
- `upsert` replaces rows with matching table-index values and appends rows with new values.

Catalog tables do not need an index for ordinary writes.
Upserts require one field in the table schema to have `rerun:is_table_index` metadata.
Choose the field that identifies rows in your table.
The `rr.SORBET_IS_TABLE_INDEX` constant provides the metadata key.
An upsert replaces the entire matching row, so omitted columns are written as null rather than retaining their previous values.

## Give columns semantic meaning

A plain Arrow field only says how a column's values are stored, such as booleans or strings.
A Rerun component type also says what the values mean, for anyone who reads the table.
The Viewer uses this meaning to pick the UI for the column, such as flag buttons for a `Flag` column and links for a `Url` column.

`table_field` on a component class returns a schema field with the component's Arrow datatype and component type.
With `editable=True`, the column can be edited in the Viewer.
Editing needs a table index and write permission.

```python
schema = pa.schema([
    pa.field("id", pa.int64(), metadata={rr.SORBET_IS_TABLE_INDEX: "true"}),
    rr.components.Url.table_field("link"),
    rr.components.Flag.table_field("reviewed", editable=True),
    rr.components.Text.table_field("notes", editable=True),
])
```

A [table blueprint](../viewer/browse-datasets/table-layout.md) can override both the component type and whether a column is editable.

## Register an existing table

Use `create_table` to create a new table managed by the catalog.
If you already have a Lance table, [`CatalogClient.register_table`](https://ref.rerun.io/docs/python/stable/catalog/#rerun.catalog.CatalogClient.register_table) makes it available in the catalog without copying its data:

```python
existing_table_url = "s3://my-bucket/tables/external_metrics.lance"
external_table = client.register_table("external_metrics", existing_table_url)
```

The URL must identify the Lance table itself rather than its parent directory.
The catalog server must have the permissions needed to access it.

## Remove a table

Call `delete()` on its `TableEntry` to remove a table from the catalog.
