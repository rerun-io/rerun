---
title: Query data out of Rerun
order: 100
---

Rerun comes with a Server + Catalog API, which enables getting data out of Rerun from code. This page provides an overview of the API, as well as recipes to load the data in popular packages such as [Pandas](https://pandas.pydata.org), [Polars](https://pola.rs), and [DuckDB](https://duckdb.org).

## The Server + Catalog API

### Starting a server with recordings

The easiest way to query data is to start a local server with your recordings:

```python
import rerun as rr

# Start a server with one or more .rrd files
with rr.server.Server(datasets={"my_dataset": ["recording.rrd"]}) as server:
    client = server.client()
    dataset = client.get_dataset("my_dataset")

    # Query the data
    df = dataset.reader(index="frame_nr")
    print(df.to_pandas())
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

### Inspecting the schema

The content of a dataset can be inspected using the `schema()` method:

```python
schema = dataset.schema()
schema.index_columns()        # list of all index columns (timelines)
schema.component_columns()    # list of all component columns
```

### Creating a filtered view

You can filter the dataset by entity paths and segments before querying:

```python
# Filter by entity paths
view = dataset.filter_contents(["/world/robot/**", "/sensors/**"])

# Filter by segment IDs (recording IDs)
view = dataset.filter_segments(["recording_001", "recording_002"])

# Chain filters
view = dataset.filter_contents(["/world/**"]).filter_segments(["recording_001"])
```

### Reading data

Once you have a dataset or filtered view, use `reader()` to create a query:

```python
# Basic query with an index
df = dataset.reader(index="frame_nr")

# Query with latest-at fill (interpolation)
df = dataset.reader(index="frame_nr", fill_latest_at=True)

# Query from a filtered view
view = dataset.filter_contents(["/world/robot/**"])
df = view.reader(index="frame_nr")
```

The `reader()` method returns a [DataFusion DataFrame](https://datafusion.apache.org/python/), which provides powerful query capabilities.

### Filtering with DataFusion

You can use DataFusion's query capabilities to filter rows:

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

### Converting to other formats

The DataFusion DataFrame can be converted to various formats:

```python
# To PyArrow Table
table = df.to_arrow_table()

# To Pandas DataFrame
pandas_df = df.to_pandas()

# To Polars DataFrame
import polars as pl
polars_df = pl.from_arrow(df.to_arrow_table())
```

## Load data to a PyArrow `Table`

```python
import rerun as rr

with rr.server.Server(datasets={"my_dataset": ["recording.rrd"]}) as server:
    dataset = server.client().get_dataset("my_dataset")
    table = dataset.reader(index="frame_nr").to_arrow_table()
```

## Load data to a Pandas dataframe

```python
import rerun as rr

with rr.server.Server(datasets={"my_dataset": ["recording.rrd"]}) as server:
    dataset = server.client().get_dataset("my_dataset")
    df = dataset.reader(index="frame_nr").to_pandas()
```

## Load data to a Polars dataframe

```python
import rerun as rr
import polars as pl

with rr.server.Server(datasets={"my_dataset": ["recording.rrd"]}) as server:
    dataset = server.client().get_dataset("my_dataset")
    df = pl.from_arrow(dataset.reader(index="frame_nr").to_arrow_table())
```

## Load data to a DuckDB relation

```python
import rerun as rr
import duckdb

with rr.server.Server(datasets={"my_dataset": ["recording.rrd"]}) as server:
    dataset = server.client().get_dataset("my_dataset")
    table = dataset.reader(index="frame_nr").to_arrow_table()
    rel = duckdb.arrow(table)
```

## Using `load_recording()` for simple cases

For simple cases where you just need to load a single recording and access its metadata (without the full query API), you can use `rr.recording.load_recording()`:

```python
import rerun as rr

recording = rr.recording.load_recording("/path/to/file.rrd")
print(f"Application ID: {recording.application_id()}")
print(f"Recording ID: {recording.recording_id()}")

# Inspect schema
schema = recording.schema()
print(schema.index_columns())
print(schema.component_columns())
```

For RRD files containing multiple recordings (e.g., with blueprints):

```python
archive = rr.recording.load_archive("/path/to/file.rrd")
print(f"The archive contains {archive.num_recordings()} recordings.")

for recording in archive.all_recordings():
    print(f"Recording: {recording.recording_id()}")
```

Note: To query data from recordings loaded this way, use the Server + Catalog API as shown above.
