---
title: Storing query results in a catalog table
order: 650
description: Summarize a dataset with a query, store the result as a catalog table, then read, join, and update it
---

This example summarizes a dataset with a query and stores the result in a [catalog table](catalog-tables.md).
It then reads the table back in a later session, joins it with the dataset, and updates it.

> [!NOTE]
> The examples on this page use a local open-source server and also run on [Rerun Hub](hub.md) ([how to switch](hub.md#run-the-examples-on-hub)).

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

The example passes `url` to choose where the local server stores the table.
See [Create a table](catalog-tables.md#create-a-table) for where a table goes without it, and on Rerun Hub.

## Access the table later

`create_table` returns a [`TableEntry`](https://ref.rerun.io/docs/python/stable/catalog/#rerun.catalog.TableEntry) that you can use immediately.
In a later session, reconnect to the catalog and retrieve the table by name with `get_table`.
Call `reader()` on the table entry to get a DataFusion dataframe that you can filter, aggregate, or join.

snippet: howto/catalog_tables[read_result]

## Join with a dataset

Pass a table as `join_meta` to [`DatasetEntry.segment_table`](https://ref.rerun.io/docs/python/stable/catalog/#rerun.catalog.DatasetEntry.segment_table) to get a dataframe of the segment table joined with the table's columns.
The table must contain a `rerun_segment_id` column.
Segments without a matching row get null values.
The join is lazy and reads the table's current rows when the result is collected.
It does not modify the dataset.

snippet: howto/catalog_tables[join_dataset]

To query only the segments that match a condition on the table, pass a filtered `reader()` dataframe of the table to [`DatasetEntry.filter_segments`](https://ref.rerun.io/docs/python/stable/catalog/#rerun.catalog.DatasetEntry.filter_segments).

## Update the table

Calling `collect()` materializes a DataFusion query as Arrow record batches in the Python process.
You can then use the table index to update existing segment summaries and append new ones:

snippet: howto/catalog_tables[update]

This query produces one row per segment, so the example uses `rerun_segment_id` as the table index, but other tables can use a different field.
See [Update a table](catalog-tables.md#update-a-table) for the other mutation operations and what an upsert requires.

## Remove the table

Call `delete()` on the `TableEntry` to remove the table from the catalog:

snippet: howto/catalog_tables[delete]
