---
title: Query data out of Rerun
order: 100
---

Rerun comes with the ability to get data out of Rerun from code. This page provides an overview of the API, as well as recipes to load the data in popular packages such as [Pandas](https://pandas.pydata.org), [Polars](https://pola.rs), and [DuckDB](https://duckdb.org).

## Starting a server with recordings

The first step to query data is to start a server and load it with a dataset containing your recording.

```python
import rerun as rr

# Start a server with one or more .rrd files
with rr.server.Server(datasets={"my_dataset": ["recording.rrd"]}) as server:
    client = server.client()
    dataset = client.get_dataset("my_dataset")
```

The server can host multiple datasets. Each dataset maps to either a list of `.rrd` files or a directory (which will be scanned for `.rrd` files):

```python
with rr.server.Server(datasets={
    # Explicit list of RRD files
    "dataset1": ["recording1.rrd", "recording2.rrd"],
    # Directory containing RRD files
    "dataset2": "/path/to/recordings_dir",
}) as server:
    client = server.client()
    # Access each dataset by name
    ds1 = client.get_dataset("dataset1")
    ds2 = client.get_dataset("dataset2")
```

When multiple recordings are loaded into a dataset, each gets mapped to a separate segment whose ID is the corresponding recording ID.

You can also start a longer running server in a separate process and connect to it by its local address.
In one file or terminal launch the server and print its address,

```python
server = rr.server.Server()
print(server.address())
```

in a separate file or terminal connect to that address

```python
client = rr.catalog.CatalogClient(server_address)
```

## Adding new datasets

New datasets can also be created or appended after the server is launched:

```python
dataset = client.create_dataset(
    name="oss_demo",
)
dataset.register(Path("/path/to/recording/recording.rrd").resolve().as_uri()).wait()
```

## Viewing datasets

Either specify the network location with the CLI at launch:

```console
rerun connect localhost:51234
```

or open the command palette in the viewer (`cmd/ctrl + P` or via the menu) and enter/select `Add Redap server`.
Set the scheme to `http` and enter the hostname and port in the dialog.

## Inspecting the schema

The content of a dataset can be inspected using the `schema()` method:

```python
schema = dataset.schema()
schema.index_columns()        # list of all index columns (timelines)
schema.component_columns()    # list of all component columns
```

## Querying a dataset using `reader`

The primary means of querying data is the `reader()` method. In its simplest form, it is used as follows:

```python
df = dataset.reader(index="frame_nr")

print(df)
```

The returned object is a [`datafusion.DataFrame`](https://datafusion.apache.org/python/autoapi/datafusion/dataframe/index.html#datafusion.dataframe.DataFrame). Rerun's query APIs heavily rely on [DataFusion](https://datafusion.apache.org), which offers a rich set of data filtering, manipulation, and conversion tools.

When calling `reader()`, an index column must be specified. It can be any of the recording's timelines. Each row of the view will correspond to a unique value of the index column. It is also possible to query the dataset using `index=None`. In this case, only the `static=True` data will be returned.

By default, when performing a query on a dataset, data for all its segments is returned. An additional `"rerun_segment_id"` column is added to the dataframe to indicate which segment each row belongs to.

An often used parameter of the `reader()` method is `fill_latest_at=True`. When used, all `null` data will be filled with a latest-at value, similarly to how the viewer works.

## Querying a subset of a dataset

In general, datasets can be arbitrarily large, and it is often useful to query only a subset of it. This is achieved using `DatasetView` objects:

```python
# Filter by entity paths
dataset_view = dataset.filter_contents(["/world/robot/**", "/sensors/**"])

# Filter by segment IDs (recording IDs)
dataset_view = dataset.filter_segments(["recording_001", "recording_002"])

# Chain filters
dataset_view = dataset.filter_contents(["/world/**"]).filter_segments(["recording_001"])
```

`DatasetView` instances have the exact same `reader()` method as the original dataset:

```python
df = dataset_view.reader(index="frame_nr")

print(df)
```

## Filtering with DataFusion

DataFusion offers a rich set of filtering, projection, and joining capabilities. Check the [DataFusion Python documentation](https://datafusion.apache.org/python/) for details.

For illustration, here are a few simple examples:

```python
from datafusion import col

df = dataset.reader(index="frame_nr")

# Filter by index range
df = df.filter(col("frame_nr") >= 0).filter(col("frame_nr") <= 100)

# Filter by column not null
df = df.filter(col("/world/robot:Position3D:positions").is_not_null())

# Select specific columns
df = df.select("frame_nr", "/world/robot:Position3D:positions")
```

## Converting to other formats

Likewise, DataFusion offers a rich set of tools to convert a dataframe to various formats.

### Load data to a PyArrow `Table`

```python
import rerun as rr

with rr.server.Server(datasets={"my_dataset": ["recording.rrd"]}) as server:
    dataset = server.client().get_dataset("my_dataset")
    table = dataset.reader(index="frame_nr").to_arrow_table()
```

### Load data to a Pandas dataframe

```python
import rerun as rr

with rr.server.Server(datasets={"my_dataset": ["recording.rrd"]}) as server:
    dataset = server.client().get_dataset("my_dataset")
    df = dataset.reader(index="frame_nr").to_pandas()
```

### Load data to a Polars dataframe

```python
import rerun as rr
import polars as pl

with rr.server.Server(datasets={"my_dataset": ["recording.rrd"]}) as server:
    dataset = server.client().get_dataset("my_dataset")
    df = pl.from_arrow(dataset.reader(index="frame_nr").to_arrow_table())
```

### Load data to a DuckDB relation

```python
import rerun as rr
import duckdb

with rr.server.Server(datasets={"my_dataset": ["recording.rrd"]}) as server:
    dataset = server.client().get_dataset("my_dataset")
    table = dataset.reader(index="frame_nr").to_arrow_table()
    rel = duckdb.arrow(table)
```
