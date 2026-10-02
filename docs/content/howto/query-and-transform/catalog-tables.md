---
title: Storing query results in catalog tables
order: 120
description: Persist derived data from catalog dataset queries
---

Catalog tables let you persist arbitrary tabular data, including results derived from queries on your datasets.
Examples include per-segment metrics, evaluation results, annotations, and processing status.
A common use case is to save these results so you can share them with colleagues, then inspect or update them later.

Catalog tables use Arrow schemas, and catalog query APIs return lazy DataFusion dataframes.
Unlike datasets, tables use _schema-on-write_: you define the table's Arrow schema before writing rows.
See the [catalog object model](../../concepts/query-and-transform/catalog-object-model.md#table-entries) for how tables relate to other catalog entries.

If you only want to display an ephemeral table in a running Viewer, see [Send tables to Rerun](../logging-and-ingestion/send-table.md) instead.

The dependencies in this example are contained in `rerun-sdk[all]`.

## Connect to a dataset

This example starts a local server with a sample dataset.
In practice, connect a `CatalogClient` to your Rerun Hub instance or another catalog server.

snippet: howto/catalog_tables[setup]

## Query the dataset

Build a DataFusion query that produces one summary row per dataset segment.
The query remains lazy until its result is written or collected.

snippet: howto/catalog_tables[query_dataset]

## Store the result

`create_table` creates an empty table from the query result's Arrow schema and returns its `TableEntry`.
[`DataFrame.write_table`](https://datafusion.apache.org/python/autoapi/datafusion/dataframe/index.html#datafusion.dataframe.DataFrame.write_table) then executes the lazy query and appends its rows to that table without first collecting the result into Python.

Catalog tables do not need an index for ordinary writes.
This example marks `rerun_segment_id` as the table index only to support the upsert demonstrated later.

snippet: howto/catalog_tables[store_result]

On a local catalog server, you can pass `url` to [`CatalogClient.create_table`](https://ref.rerun.io/docs/python/stable/catalog/#rerun.catalog.CatalogClient.create_table) to choose the storage directory.
If you omit it, the server uses its configured storage.
Rerun Hub always uses its configured storage instead, so do not pass `url`.

## Access the table later

`create_table` returns a [`TableEntry`](https://ref.rerun.io/docs/python/stable/catalog/#rerun.catalog.TableEntry) that you can use immediately.
In a later session, reconnect to the catalog and retrieve the table by name with `get_table`.
Call `reader()` on the table entry to get a DataFusion dataframe that you can filter, aggregate, or join.

snippet: howto/catalog_tables[read_result]

## Update a table

`write_table` operates on a DataFusion dataframe.
For direct updates, a `TableEntry` supports three mutation operations using Arrow record batches:

- `append` adds rows to the existing table.
- `overwrite` replaces all existing rows.
- `upsert` replaces rows with matching table-index values and appends rows with new values.

Calling `collect()` materializes a DataFusion query as Arrow record batches in the Python process.
You can then use the table index to update existing segment summaries and append new ones:

snippet: howto/catalog_tables[update]

Upserts require one field in the table schema to have `rerun:is_table_index` metadata.
Choose the field that identifies rows in your table.
This query produces one row per segment, so the example uses `rerun_segment_id`, but other tables can use a different field.
The `rr.SORBET_IS_TABLE_INDEX` constant provides the metadata key.
An upsert replaces the entire matching row, so omitted columns are written as null rather than retaining their previous values.

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

Call `delete()` on its `TableEntry` to remove a table from the catalog:

snippet: howto/catalog_tables[delete]
