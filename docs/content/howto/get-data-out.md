---
title: Query data out of Rerun
order: 100
---

Rerun comes with a Dataframe API, which enables getting data out of Rerun from code. This page provides an overview of the API, as well as recipes to load the data in popular packages such as [Pandas](https://pandas.pydata.org), [Polars](https://pola.rs), and [DuckDB](https://duckdb.org).

## The dataframe API

### Loading a recording

A recording can be loaded from a RRD using the `load_recording()` function:

```python
import rerun as rr

recording = rr.dataframe.load_recording("/path/to/file.rrd")
```

Although RRD files generally contain a single recording, they may occasionally contain 2 or more. This can happen, for example, if the RRD includes a blueprint, which is stored as a recording that is separate from the data.

For such RRD, the `load_archive()` function can be used:

<!-- NOLINT_START -->

```python
import rerun as rr

archive = rr.dataframe.load_archive("/pat/to/file.rrd")

print(f"The archive contains {archive.num_recordings()} recordings.")

for recording in archive.all_recordings():
    ...
```

<!-- NOLINT_END -->

The overall content of the recording can be inspected using the `schema()` method:

```python
schema = recording.schema()
schema.index_columns()        # list of all index columns (timelines)
schema.component_columns()    # list of all component columns
```

### Creating a view

The first step for getting data out of a recording is to create a view, which requires specifying an index column and what content to include.

As of Rerun 0.19, views must have exactly one index column, which can be any of the recording timelines.
Each row of the view will correspond to a unique value of the index column.
If a row has a `null` in the returned index (time) column, it means that data was static.
In the future, it will be possible to have other kinds of column as index, and more than a single index column.

The `contents` define which columns are included in the view and can be flexibly specified as entity expression,
optionally providing a corresponding list of components.

These are all valid ways to specify view content:

```python
# everything in the recording
view = recording.view(index="frame_nr", contents="/**")

# everything in the recording, except the /world/robot subtree
view = recording.view(index="frame_nr", contents="/**\n- /world/robot/**")

# all `Scalar` components in the recording
view = recording.view(index="frame_nr", contents={"/**": ["Scalar"]})

# some components in an entity subtree and a specific component
# of a specific entity
view = recording.view(index="frame_nr", contents={
    "/world/robot/**": ["Position3D", "Color"],
    "/world/scene": ["Text"],
})
```

### Filtering rows in a view

A view has several APIs to further filter the rows it will return.

#### Filtering by time range

Rows may be filtered to keep only a given range of values from its index column:

```python
# only keep rows for frames 0 to 10
view = view.filter_range_sequence(0, 10)
```

This API exists for both temporal and sequence timeline, and for various units:

- `view.filter_range_sequence(start_frame, end_frame)` (takes `int` arguments)
- `view.filter_range_secs(stat_second, end_second)` (takes `float` arguments)
- `view.filter_range_nanos(start_nano, end_nano)` (takes `int` arguments)

(all ranges are including both start and end values)

#### Filtering by index value

Rows may be filtered to keep only those whose index corresponds to a specific set of value:

```python
view = view.filter_index_values([0, 5, 10])
```

Note that a precise match is required.
Since Rerun internally stores times as `int64`, this method is only available for integer arguments (nanos or sequence number).
Floating point seconds would risk false mismatch due to numerical conversion.

##### Filtering by column not null

Rows where a specific column has null values may be filtered out using the `filter_is_not_null()` method. When using this method, only rows for which a logging event exist for the provided column are returned.

```python
# only keep rows where a position is available for the robot
view = view.filter_is_not_null("/world/robot:Position3D")
```

### Specifying rows

Instead of filtering rows based on the existing data, it is possible to specify exactly which rows must be returned by the view using the `using_index_values()` method:

```python
# resample the first second of data at every millisecond
view = view.using_index_values(range(0, 1_000_000, 1_000_0000_000))
```

In this case, the view will return rows in multiples of 1e6 nanoseconds (i.e. for each millisecond) over a period of one second.
A precise match on the index value is required for data to be produced on the row.
For this reason, a floating point version of this method is not provided for this feature.

Note that this feature is typically used in conjunction with `fill_latest_at()` (see next paragraph) to enable arbitrary resampling of the original data.

### Filling empty values with latest-at data

By default, the rows returned by the view may be sparse and contain values only for the columns where a logging event actually occurred at the corresponding index value.
The view can optionally replace these empty cells using a latest-at query. This means that, for each such empty cell, the view traces back to find the last logged value and uses it instead. This is enabled by calling the `fill_latest_at()` method:

```python
view = view.fill_latest_at()
```

### Reading the data

Once the view is fully set up (possibly using the filtering features previously described), its content can be read using the `select()` method. This method optionally allows specifying which subset of columns should be produced:

```python
# select all columns
record_batches = view.select()

# select only the specified columns
record_batches = view.select(
    [
        "frame_nr",
        "/world/robot:Position3D",
    ],
)
```

The `select()` method returns a [`pyarrow.RecordBatchReader`](https://arrow.apache.org/docs/python/generated/pyarrow.RecordBatchReader.html), which is essentially an iterator over a stream of [`pyarrow.RecordBatch`](https://arrow.apache.org/docs/python/generated/pyarrow.RecordBatch.html#pyarrow-recordbatch)es containing the actual data. See the [PyArrow documentation](https://arrow.apache.org/docs/python/index.html) for more information.

For the rest of this page, we explore how these `RecordBatch`es can be ingested in some of the popular data science packages.

## Load data to a PyArrow `Table`

The `RecordBatchReader` provides a [`read_all()`](https://arrow.apache.org/docs/python/generated/pyarrow.RecordBatchReader.html#pyarrow.RecordBatchReader.read_all) method which directly produces a [`pyarrow.Table`](https://arrow.apache.org/docs/python/generated/pyarrow.Table.html#pyarrow.Table):

```python
import rerun as rr

recording = rr.dataframe.load_recording("/path/to/file.rrd")
view = recording.view(index="frame_nr", contents="/**")

table = view.select().read_all()
```

## Load data to a Pandas dataframe

The `RecordBatchReader` provides a [`read_pandas()`](https://arrow.apache.org/docs/python/generated/pyarrow.RecordBatchReader.html#pyarrow.RecordBatchReader.read_pandas) method which returns a [Pandas dataframe](https://pandas.pydata.org/pandas-docs/stable/reference/api/pandas.DataFrame.html):

```python
import rerun as rr

recording = rr.dataframe.load_recording("/path/to/file.rrd")
view = recording.view(index="frame_nr", contents="/**")

df = view.select().read_pandas()
```

## Load data to a Polars dataframe

A [Polars dataframe](https://docs.pola.rs/api/python/stable/reference/dataframe/index.html) can be created from a PyArrow table:

```python
import rerun as rr
import polars as pl

recording = rr.dataframe.load_recording("/path/to/file.rrd")
view = recording.view(index="frame_nr", contents="/**")

df = pl.from_arrow(view.select().read_all())
```

## Load data to a DuckDB relation

A [DuckDB](https://duckdb.org) relation can be created directly using the `pyarrow.RecordBatchReader` returned by `select()`:

```python
import rerun as rr
import duckdb

recording = rr.dataframe.load_recording("/path/to/file.rrd")
view = recording.view(index="frame_nr", contents="/**")

rel = duckdb.arrow(view.select())
```
