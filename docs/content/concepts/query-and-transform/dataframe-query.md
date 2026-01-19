---
title: Dataframe query
order: 100
---

Robotic and sensor data is inherently messy:
- Sensors operate at different rates, producing unaligned data streams
- Data is sparse: not every component has a value at every timestamp
- Multiple timelines coexist: wall clock time, frame numbers, sensor ticks, etc.

ML workloads, on the other hand, need clean tabular data: aligned rows where each row represents one sample, with a consistent schema and a single index.

Dataframe queries are designed to bridge this gap. They allow you to query arbitrary Rerun data and produce a dataframe as output.

## Where can dataframe queries be used?

Dataframe queries can be used in two contexts:

- **In the Viewer**: The [dataframe view](../../reference/types/views/dataframe_view.md) displays query results as a table, useful for inspecting raw values and debugging.
- **Via the Catalog SDK**: The [`DatasetEntry`](https://ref.rerun.io/docs/python/stable/common/catalog/#rerun.catalog.DatasetEntry) object provides [`reader()`](https://ref.rerun.io/docs/python/stable/common/catalog/#rerun.catalog.DatasetEntry.reader) and filtering methods for programmatic access.


## Understanding dataframe queries

Let's use an example to illustrate how dataframe queries work.

Dataframe queries run against datasets stored on a [Data Platform](../how-does-rerun-work.md#data-platform), an open-source implementation of which ships with Rerun.
We can create a demo recording and load it into a temporary local catalog using the following code:

snippet: concepts/query-and-transform/dataframe_query[setup]


We can then perform a dataframe query:

snippet: concepts/query-and-transform/dataframe_query[query]


This should produce an output similar to:

```
┌──────────────────────────────────┬────────────────────┬───────────────────────────────────┐
│ rerun_segment_id                 ┆ step               ┆ /data:Scalars:scalars             │
│ ---                              ┆ ---                ┆ ---                               │
│ type: Utf8                       ┆ type: nullable i64 ┆ type: nullable List[nullable f64] │
│                                  ┆ index_name: step   ┆ archetype: Scalars                │
│                                  ┆ kind: index        ┆ component: Scalars:scalars        │
│                                  ┆                    ┆ component_type: Scalar            │
│                                  ┆                    ┆ entity_path: /data                │
│                                  ┆                    ┆ kind: data                        │
╞══════════════════════════════════╪════════════════════╪═══════════════════════════════════╡
│ 5712205b356b470e8d1574157e55f65e ┆ 13                 ┆ [0.963558185417193]               │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ 5712205b356b470e8d1574157e55f65e ┆ 14                 ┆ [0.9854497299884601]              │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ 5712205b356b470e8d1574157e55f65e ┆ 15                 ┆ [0.9974949866040544]              │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ 5712205b356b470e8d1574157e55f65e ┆ 16                 ┆ [0.9995736030415051]              │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ 5712205b356b470e8d1574157e55f65e ┆ 17                 ┆ [0.9916648104524686]              │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ 5712205b356b470e8d1574157e55f65e ┆ 18                 ┆ [0.9738476308781951]              │
└──────────────────────────────────┴────────────────────┴───────────────────────────────────┘
```

Let's unpack what happens here:
- **Catalog required**: We use `rr.server.Server()` to spin up a temporary local catalog. In production, you might connect to a Rerun Data Platform deployment instead. We then obtain the dataset to be queried from the catalog.
- **Content filtering**: The `filter_contents()` method restricts the scope of the query to specific entities. This affects which columns are returned, but may also change which rows are returned since rows are only produced where at least one filtered column has data (see [How are rows produced?](#how-are-rows-produced-by-dataframe-queries)).
- **Reader produces a lazy dataframe**: The `reader(index=…)` method returns a [DataFusion](https://datafusion.apache.org/) dataframe. The `index` parameter specifies which timeline drives row generation: a row is produced for each unique value of this index where data exists. The returned dataframe is lazy and doesn't execute until it is collected.
- **Filtering/aggregation/joining/etc.**: The standard suite of dataframe operations is provided by DataFusion. Here we use `filter()` to filter rows based on the data. Again, these are lazy operations that only build a query plan.
- **Execution**: The `print(df)` implicitly executes the dataframe's query plan and returns the final result. The same would happen when converting to dataframe for other frameworks (Pandas, Polars, PyArrow, etc.).

```d2
direction: down

Dataset: {
  shape: cylinder
}

view: {
  label: "Dataset view"
}

DataFrame: {
  label: "DataFusion DataFrame"
}

Result: {
  label: "Materialized rows\n(Arrow RecordBatch)"
  shape: page
}


Dataset -> view: "filter_contents()\nfilter_segments()"
Dataset -> DataFrame: "reader()"
view -> DataFrame: "reader()"
DataFrame -> Result: "collect()"
```


## FAQ

### How are rows produced by dataframe queries?

A row is produced for each distinct index (or timeline) value for which there is at least one value in the filtered content

For example, if you filter for entities `/camera` and `/lidar`, and `/camera` has data at timestamps [1, 2, 3] while `/lidar` has data at [2, 4], the output will have rows for timestamps [1, 2, 3, 4]. Columns without data at a given timestamp will contain null values (unless sparse fill is enabled).


### What is the difference between dataset's `filter_contents()` and DataFusion's `select()`?

At first glance, both methods control which columns appear in the result. However, they differ in an important way:

- **`filter_contents()`** restricts which entities are considered for row generation. This affects both which columns *and* which rows are returned.
- **`select()`** is a DataFusion operation that only filters columns *after* rows have been determined. It does not affect row generation.

Building on the previous example, if `/camera` has data at timestamps [1, 2, 3] and `/lidar` has data at [2, 4]:

```python
# Rows at [1, 2, 3] with only /camera columns
dataset.filter_contents("/camera").reader(index="timestamp")

# Rows at [1, 2, 3, 4] with only /camera columns
# (null values at timestamp 4 where /camera has no data)
dataset.filter_contents(["/camera", "/lidar"]).reader(index="timestamp").select("/camera")
```

### How are segments handled by dataframe queries?

When querying a dataset with multiple [segments](catalog-object-model.md#segments), the query is applied on a segment-by-segment basis. This means:

- Latest-at semantics do not cross segment boundaries. Each segment is queried independently.
- The output includes a `rerun_segment_id` column identifying which segment each row comes from.
- Use `filter_segments()` on a dataset or dataset view to restrict the query to specific segment IDs.

### How is static data queried?

[Static data](../logging-and-ingestion/timelines.md#static-data) has no associated timeline and represents values that don't change over time.

In regular queries, static columns appear in every row with their constant value.

To query *only* static data, set the index to `None`:

```python
df = dataset.reader(index=None)
```

The returned dataframe contains a single row with all static data. This is useful for retrieving configuration, calibration data, or other time-invariant information.


### How do dataframe queries achieve resampling?

By default, rows are produced only at index values where data exists. To sample at specific timestamps (even if no data exists there), use the `using_index_values` parameter combined with `fill_latest_at=True`:

```python
# Sample at fixed 10Hz (100ms intervals)
timestamps = np.arange(start_time, end_time, np.timedelta64(100, "ms"))
df = dataset.reader(
    index="timestamp",
    using_index_values=timestamps,
    fill_latest_at=True,
)
```

- `using_index_values` specifies the exact timestamps to sample
- `fill_latest_at=True` fills null values with the most recent data (latest-at semantics)

For a complete example, see the [Time-align data](../../howto/query-and-transform/time_alignment.md) how-to.


## Additional resources

- [🐍 Python Catalog SDK reference](https://ref.rerun.io/docs/python/stable/common/catalog/)
- [Dataframe view](../../reference/types/views/dataframe_view.md) for visualizing query results in the Viewer
- [Query semantics & partial updates](../logging-and-ingestion/latest-at.md) for understanding latest-at and range queries
