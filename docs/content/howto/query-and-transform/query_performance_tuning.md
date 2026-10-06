---
title: Query Performance Tuning
order: 110
description: Tips for keeping queries fast as datasets grow
---

This is a loose collection of considerations when querying Rerun datasets.
Over time baseline performance will improve, rendering some of these approaches unnecessary.
Since Rerun depends on [DataFusion](https://datafusion.apache.org/), some of these approaches are observations from our own usage.

First, generate a DataFrame for comparison:

snippet: howto/dataframe_performance[get_df]

## Extract Python types from a DataFrame

DataFusion is a streaming query engine, which allows for processing arbitrarily large amounts of data.
When working with smaller or filtered-down datasets that fit into memory, you can extract data into Python variables for further post processing.
In these examples, we convert DataFrames to [PyArrow](https://arrow.apache.org/docs/python/index.html) tables to materialize them in memory.
Similar patterns using Polars or Pandas also apply.

### Prefer to_numpy

This is technically a [PyArrow](https://arrow.apache.org/docs/python/index.html) and general Python detail.
For example, when extracting data from a PyArrow table, `to_pylist` can be multiple orders of magnitude slower, even when using `to_numpy(zero_copy_only=False)`.

snippet: howto/dataframe_performance[to_list_bad]


## Fine-tune data collection

Similar to the approach described above to collect a DataFusion `DataFrame` into a PyArrow table, you can instead collect the results in memory and keep them as a `DataFrame`.
Then any operations on this in-memory (cached) `DataFrame` are typically _very_ fast.

snippet: howto/dataframe_performance[cache]

## Where a query runs

`reader()` returns a DataFusion dataframe backed by a Rerun table provider.
DataFusion plans the query and asks the provider which filters it can push to the server.
The provider pushes segment, entity and time filters.
The server uses the chunk index to return the chunks that can match, and DataFusion runs the rest of the plan on those chunks, on the client.

The server applies these filters:

| Filter | How to write it |
| --- | --- |
| Segment | `filter_segments()`, or `rerun_segment_id` with `==` or `IN` |
| Entity | `filter_contents()`, and the selected columns when `fill_latest_at` is off |
| Time | the index column with `=`, `<`, `<=`, `>`, `>=`, `BETWEEN` or `IN` against a literal, or `using_index_values` |

The server filters whole chunks, so the chunks it returns can hold rows outside the filter.
DataFusion filters the rows again, so the result is exact.

The server compares only the bare index column with a literal.
`col("time").cast(pa.int64()) >= t0` is not pushed: the client fetches every chunk of the selected entities and compares the rows.

`is_not_null()` on a component column makes the reader produce rows only where that component has data.
It does not reduce the chunks fetched.

Everything else runs in DataFusion on the fetched rows: filters on data columns, aggregates, joins and sorts.
`LIMIT` stops the fetch early but does not reach the server.
With `fill_latest_at=True` the selected columns no longer narrow entities, because the other entities' timestamps still produce filled rows.
Use `filter_contents()` instead.

`segment_table()` reads the index and fetches no chunks.
Use it for per-segment facts such as duration, counts and property values.

## Leverage sparsity to minimize scans
In a write once, read many paradigm adding an additional sparse column can enable cheap access to data of interest via filtering.
Read the sparse column on its own first, which fetches only its chunks, then query the heavy data at its timestamps with `using_index_values`.
For example, `Mp4Reader` logs `VideoStream:is_keyframe` with only `true` rows, so on Rerun Hub selecting just that column finds the keyframes without fetching any video samples.

snippet: howto/dataframe_performance[sparsity]
