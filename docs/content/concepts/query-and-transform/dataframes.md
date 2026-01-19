---
title: Dataframes
order: 100
---

Rerun stores time-series data: [entities](../logging-and-ingestion/entity-component.md) with [components](../logging-and-ingestion/entity-component.md) that change over [time](../logging-and-ingestion/timelines.md).
The dataframe API lets you query this data as tabular [Arrow](https://arrow.apache.org/) data, which can then be used with tools like [pandas](https://pandas.pydata.org/), [Polars](https://pola.rs/), or any other Arrow-compatible library.

For API details, see the dataframe reference for [🐍 Python](https://ref.rerun.io/docs/python/stable/common/dataframe/) and [🦀 Rust](https://docs.rs/rerun/latest/rerun/dataframe/index.html).
For a hands-on introduction, check out [our Getting Started guide](../../getting-started/data-out) or the [How-To on getting data out](../../howto/query-and-transform/get-data-out.md).


## Bridging robotic data and ML workloads

Robotic and sensor data is inherently messy:
- Sensors fire at different rates, producing unaligned data streams
- Data is sparse: not every component has a value at every timestamp
- Multiple timelines coexist: wall clock time, frame numbers, sensor ticks

ML workloads, on the other hand, need clean tabular data: aligned rows where each row represents one sample, with a consistent schema and a single index.

The dataframe API bridges this gap:
- **Latest-at semantics** align disparate sensor streams to a common timeline. For example: "what was the latest camera frame when this lidar scan arrived?"
- **Sparse fill** handles gaps by propagating the most recent value forward
- **Filtering** selects relevant time ranges, entities, and components
- **Sampling** retrieves data at specific timestamps

For more on latest-at vs. range query semantics, see [Query semantics & partial updates](../logging-and-ingestion/latest-at.md).


## Query model

A dataframe query has four stages:

1. **View contents**: which columns (entity paths and components) to include
2. **Filters**: which rows to include, based on a timeline index and time range
3. **Samplers**: sample rows at specific index values (can create new rows via interpolation)
4. **Selection**: final column selection from the filtered results

A rough mental model in SQL terms:
```
SELECT <selection> FROM <view_contents> WHERE <filters>
```

The output is an Arrow table where:
- Columns are timelines (indexes) followed by components
- Rows are points in time, filtered by the query parameters


## Querying static data

[Static data](../logging-and-ingestion/timelines.md#static-data) has no associated timeline: it represents values that don't change over time.

To query only static data, set the index to `None`:

```python
view = recording.view(index=None, contents="/**")
```

The returned dataframe contains a single row with all static data.
This is useful for retrieving configuration, calibration data, or other time-invariant information.


## Using the dataframe API

The following snippet demonstrates how to query the first 10 rows in a Rerun recording:

snippet: reference/dataframe_query

> To run this example, you'll need an RRD file. Either use one of yours, or grab an example:
> ```
> $ curl 'https://app.rerun.io/version/latest/examples/dna.rrd' -o /tmp/dna.rrd
> ```

Check out the API reference to learn more about all the ways that data can be searched and filtered:
- [🐍 Python API reference](https://ref.rerun.io/docs/python/stable/common/dataframe/)
  - [Example](https://github.com/rerun-io/rerun/blob/latest/examples/python/dataframe_query/dataframe_query.py)
- [🦀 Rust API reference](https://docs.rs/rerun/latest/rerun/dataframe/index.html)
  - [Example](https://github.com/rerun-io/rerun/blob/latest/examples/rust/dataframe_query/src/main.rs)


## Dataframe view in the Viewer

The Viewer can also display data as a table using the dataframe view.
See [Dataframe view](../visualization/dataframe-view.md) for details on configuring it via the blueprint API or the UI.
