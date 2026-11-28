---
title: Query data out of Rerun
order: 300
description: Query Rerun data from code and load it into Pandas, Polars, or DuckDB
---

This page gives an overview of the API for getting data out of Rerun from code.
It also has recipes to load the data in packages such as [Pandas](https://pandas.pydata.org), [Polars](https://pola.rs), and [DuckDB](https://duckdb.org).

> [!NOTE]
> The examples on this page use a local open-source server and also run on [Rerun Hub](hub.md) ([how to switch](hub.md#run-the-examples-on-hub)).

To read an `.rrd` file in Python, you can either load it into a local catalog server (described first on this page), or [read it without a server](#reading-an-rrd-file-without-a-server).
To read it from Rust, see [Querying from Rust](#querying-from-rust).

## Starting a server with recordings

The first step to query data is to start a catalog server and load it with a dataset containing your recording.

See the [catalog object model](catalog-object-model.md) docs for more details on how datasets are organized in Rerun.

snippet: howto/get_data_out[start_server]

The server can host multiple datasets. Each dataset maps to either a list of `.rrd` files or a directory (which will be scanned for `.rrd` files):

snippet: howto/get_data_out[multiple_datasets]

When multiple recordings are loaded into a dataset, each gets mapped to a separate segment whose ID is the corresponding recording ID.

You can also start a longer running server in a separate process and connect to it by its local address.
In one file or terminal, launch the server on a fixed port and keep the process running:

```python
server = rr.server.Server(port=51234)
input("Server running, press Enter to stop it")
```

In a separate file or terminal, connect to it:

```python
client = rr.catalog.CatalogClient("rerun+http://127.0.0.1:51234")
```

## Adding new datasets

New datasets can also be created or appended after the server is launched:

```python
dataset = client.create_dataset(
    name="oss_demo",
)
dataset.register([Path("/path/to/recording/recording.rrd").resolve().as_uri()]).wait()
```

## Viewing datasets

Either specify the network location with the CLI at launch:

```console
rerun rerun+http://localhost:51234
```

or open the command palette in the viewer (`cmd/ctrl + K` or via the menu) and pick "Connect to a server…".
Set the scheme to `http` and enter the hostname and port in the dialog.

## Getting an overview

Before querying, `info()` summarizes what a dataset holds: its segments, layers, timeline ranges, contents and property columns.
It reads only the segment table and the schema, so it stays fast on large datasets.

```python
>>> dataset.info()
DatasetInfo(robot_episodes: 3 segments, 41.1 KiB)
  contents: 3 entities, 3 component columns
  properties: property:RecordingInfo:start_time, property:episode:operator, property:episode:success
  layers:
    base: in 3 segments
    predictions: in 2 segments
  timelines:
    capture_time: timestamp, 2026-03-01 09:00:00Z … 2026-03-01 11:02:19Z, in 3 segments
    frame: sequence, #0 … #139, in 3 segments
```

The same information is available as fields, e.g. `{layer.name: layer.num_segments for layer in dataset.info().layers}`.
See [`DatasetInfo`](https://ref.rerun.io/docs/python/stable/catalog/#rerun.catalog.DatasetInfo) for the full list.

A selection has the same method, e.g. `dataset.filter_segments(segment_ids).filter_contents("/camera/**").info()`.

## Inspecting the schema

The content of a dataset can be inspected using the `schema()` method:

```python
schema = dataset.schema()
schema.index_columns()  # list of all index columns (timelines)
schema.component_columns()  # list of all component columns
schema.column_names()  # list of all column names
schema.to_arrow()  # convert to a PyArrow schema
```

To inspect one segment's schema, use:

```python
segment_schema = dataset.segment(segment_id).schema()
```

Referenced assets are excluded from the segment's schema.

Component column names follow the pattern `$entity_path:$Archetype:$field`, e.g. `/robot/gripper/force:Scalars:scalars`.
Data logged without an archetype, such as with `rr.AnyValues`, omits the `$Archetype` part, e.g. `/robot/meta:mode`.

## Querying a dataset using `reader`

The primary means of querying data is the `reader()` method. In its simplest form, it is used as follows:

```python
df = dataset.reader(index="frame_nr")

print(df)
```

Printing a DataFrame, whether with `print(df)` or `df.show()`, prints every byte of blob columns such as images and video.
Exclude blob columns with `filter_contents()` or `select()` before printing.

The returned object is a [`datafusion.DataFrame`](https://datafusion.apache.org/python/autoapi/datafusion/dataframe/index.html#datafusion.dataframe.DataFrame).
Rerun's query APIs rely on [DataFusion](https://datafusion.apache.org) for data filtering, manipulation, and conversion.

When calling `reader()`, an index column must be specified. It can be any of the recording's timelines. Each row of the view will correspond to a unique value of the index column. It is also possible to query the dataset using `index=None`. In this case, only the `static=True` data will be returned.

`log_time` is added automatically by `rr.log()` and records when each log call was made.
`send_columns()` does not add it, so querying that data with `index="log_time"` returns no rows.

By default, when performing a query on a dataset, data for all its segments is returned. An additional `"rerun_segment_id"` column is added to the dataframe to indicate which segment each row belongs to.

An often used parameter of the `reader()` method is `fill_latest_at=True`. When used, all `null` data will be filled with a latest-at value, similarly to how the viewer works.
With `fill_latest_at=True`, the result also has a row at each timestamp of the other entities in the view, filled with the latest values.
Those entities are fetched even if you don't select their columns, so restrict the view with `filter_contents()`.
See [Where a query runs](../query-and-transform/dataframes/query_performance_tuning.md#where-a-query-runs) for what narrows a query.

## Querying a subset of a dataset

Datasets can be arbitrarily large, and it is often useful to query only a subset of it.
This is achieved using `DatasetSelection` objects:

snippet: howto/get_data_out[filter_selection]

`filter_contents()` takes [entity path filter](../log/data-model/entity-path.md#entity-path-filters) expressions.

Chained filters narrow the selection: an entity or segment is kept only if every call selects it.
To combine several subtrees, or includes with excludes, pass all the expressions to a single `filter_contents()` call.
To exclude entities from an existing selection, start from everything and exclude from there.

snippet: howto/get_data_out[filter_semantics]

`DatasetSelection` instances have the exact same `reader()` method as the original dataset:

```python
df = selection.reader(index="frame_nr")

print(df)
```

## Streaming Arrow batches

To process a dataset as one table of component columns aligned on a timeline, call `read_arrow()` to directly stream PyArrow record batches.
Rows are ordered by segment ID, then by index within each segment; equal keys have no guaranteed order.

For example, select one segment and process each batch as it arrives:

```python
selection = dataset.filter_segments("my_segment_id").filter_contents("/world/robot/**")

for batch in selection.read_arrow(index="frame_nr"):
    print(batch.num_rows)
```

`read_arrow()` returns a PyArrow `RecordBatchReader` that you can iterate over.
Calling `read_all()` on the reader collects all remaining batches in memory.
Streaming can still download all matching chunks, so restrict segments and entities or use `reader()` for component projection and time filtering.

## Filtering with DataFusion

DataFusion also supports filtering, projection, and joins.
See the [DataFusion Python documentation](https://datafusion.apache.org/python/) for details.

For illustration, here are a few simple examples:

snippet: howto/get_data_out[datafusion_filter]

## Querying with SQL

The dataframes returned by `reader()` belong to the client's DataFusion session, `client.ctx`.
To query one with SQL, register it as a view:

```python
ctx = client.ctx
ctx.register_view("robot", dataset.filter_contents("/robot/**").reader(index="frame_nr"))

df = ctx.sql('SELECT rerun_segment_id, frame_nr, "/robot/gripper/force:Scalars:scalars"[1] AS force FROM robot')
```

Column names must be double-quoted.
Component columns are [lists](../query-and-transform/dataframes/dataframe-queries.md#why-are-component-columns-lists), and SQL list indices start at 1.
Catalog tables can be queried by name without registering them, e.g. `SELECT * FROM "my_table"`.

### Query across datasets

Dataframes from different datasets on the same server share `client.ctx`, so you can register each one as a view and combine them in one SQL query:

snippet: howto/get_data_out[cross_datasets]

`UNION ALL` needs the same columns on both sides, so select them explicitly.
You can also `JOIN` the views, for example on `rerun_segment_id` and the index column.

## Converting to other formats

DataFusion can also convert a dataframe to other formats.
Converting executes the query and loads the full result into memory.
Filter and aggregate in DataFusion first, so that only the rows you need are converted.
See [Common Dataframe Operations](../query-and-transform/dataframes/dataframe_operations.md) for aggregation examples.

### Load data to a PyArrow `Table`

snippet: howto/get_data_out[to_arrow]

### Load data to a Pandas dataframe

snippet: howto/get_data_out[to_pandas]

### Load data to a Polars dataframe

snippet: howto/get_data_out[to_polars]

### Load data to a DuckDB relation

```python
import rerun as rr
import duckdb

with rr.server.Server(datasets={"my_dataset": ["recording.rrd"]}) as server:
    dataset = server.client().get_dataset("my_dataset")
    table = dataset.reader(index="frame_nr").to_arrow_table()
    rel = duckdb.arrow(table)
```

### Export to CSV

Convert to Pandas, then use its `to_csv()` method:

```python
dataset.reader(index="frame_nr").to_pandas().to_csv("recording.csv", index=False)
```

Component columns are [lists](../query-and-transform/dataframes/dataframe-queries.md#why-are-component-columns-lists), which CSV has no type for, so they are written as text.
To get one value per cell, select the list elements first, for example with SQL as shown in [Querying with SQL](#querying-with-sql).
Exclude image and video columns, since their blobs can be very large.

## Reading an `.rrd` file without a server

The [Chunk Processing API](../ingest/chunk-processing-api.md) can read an `.rrd` file directly with `RrdReader`.
Collect its chunks into a `ChunkStore`, and query that with the same `reader()` method as a dataset:

snippet: howto/get_data_out[rrd_reader]

`collect()` loads the whole recording into memory.
The result has no `rerun_segment_id` column, since there is only one recording.
If the file holds several recordings, pick one with `RrdReader(…).recordings()` and the `store` argument of `stream()`.

## Querying from Rust

The Rust SDK can load an `.rrd` file and query it with `QueryEngine`, in the `rerun::dataframe` module (enabled by the default `dataframe` feature):

snippet: howto/query_rrd_file[query]

`from_rrd_filepath` returns one engine per store in the file, including blueprints, which is why the loop skips everything but recordings.
For a complete program, see the [`dataframe_query` example](https://github.com/rerun-io/rerun/tree/main/examples/rust/dataframe_query).
