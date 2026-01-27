---
title: Data Platform Performance
order: 100
---

This is a loose collection of considerations when using the Rerun Data Platform.
Over time baseline performance will improve, rendering some of these approaches to be unnecessary.
Since Rerun depends on [DataFusion](https://datafusion.apache.org/) some of these approaches are observation in our own usage.

## Extracting python types from dataframe

### Prefer to_numpy
This is technically a [pyarrow](https://arrow.apache.org/docs/python/index.html) and general python detail.
If extracting data `to_list` can be multiple orders of magnitude slower even if using `zero_copy_only=False`

<!-- TODO: Insert snippet -->
```python
pa.table(df)["col_name"].to_list()
# vs
pa.table(df)["col_name"].to_list()
```

### Beware to_py for timestamps
Python's default timestamp from [datetime](https://docs.python.org/3/library/datetime.html#available-types) only supports microsecond resolution.
If pandas is installed datafusion can return a [pandas.Timestamp](https://pandas.pydata.org/docs/reference/api/pandas.Timestamp.html) which supports nanoseconds.
Or using numpy you can retrieve a [numpy.datetime64](https://numpy.org/doc/stable/reference/arrays.scalars.html#numpy.datetime64) which also supports nanoseconds.

<!-- TODO: Insert snippet -->
```python
pa.table(df)["real_time"].to_py()
# I forget how to do the pandas bit
pa.table(df)["real_time"].to_numpy()
```

## Latency sensitivity
If you are latency sensitive then DataFusion's lazy execution can avoid round trips to the server.

# TODO with the new api might need to check this and adjust the example
# basically reduce reader calls
```python
dataset.reader(index="real_time")
for segment in segments:
    tbl = pa.table(dataset.filter(col("rerun_segment_id")==segment))
# vs
for some_filter in filters:
    tbl = pa.table(dataset.filter(some_filter).reader(index="real_time"))
```

## Fine tuning collection
DataFusion attempts to make optimized plans for pulling data.
However, using `cache` effectively can significantly improve performance if storing expensive operations.

<!-- TODO: Insert snippet -->
```python
df = old_df.some_expensive_function()
cache_df = df
# compare doing 2 more steps or something
df.count()
cache_df.count()
```

## Leverage sparsity to minimize scans
In a write once, read many paradigm adding an additional sparse column can be an incredibly cheap way to minimize data access.

<!-- TODO: Insert snippet -->
```python
df = # dataframe with 3 columns, 1 with some aggregation id, 1 with random floats and one with random words
# Show query for grabbing the maximum random float by aggregation id and returning the corresponding word

# Then show adding a column called max_val with a single bool.
# Then show how we can use that column to extract the timestamp across aggregation ids
# to directly return the word without loading the full dataset
```
