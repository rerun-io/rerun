---
title: Query Performance Tuning
order: 110
---

This is a loose collection of considerations when querying Rerun datasets.
Over time baseline performance will improve, rendering some of these approaches unnecessary.
Since Rerun depends on [DataFusion](https://datafusion.apache.org/), some of these approaches are observations from our own usage.

First, generate a DataFrame for comparison:

snippet: reference/dataframe_performance[get_df]

## Extract Python types from a DataFrame

DataFusion is a streaming query engine, which allows for processing arbitrarily large amounts of data.
When working with smaller or filtered-down datasets that fit into memory, you can extract data into Python variables for further post processing.
In these examples, we convert DataFrames to [PyArrow](https://arrow.apache.org/docs/python/index.html) tables to materialize them in memory.
Similar patterns using Polars or Pandas also apply.

### Prefer to_numpy

This is technically a [PyArrow](https://arrow.apache.org/docs/python/index.html) and general Python detail.
For example, when extracting data from a PyArrow table, `to_pylist` can be multiple orders of magnitude slower, even when using `to_numpy(zero_copy_only=False)`.

snippet: reference/dataframe_performance[to_list_bad]


## Fine-tune data collection

Similar to the approach described above to collect a DataFusion `DataFrame` into a PyArrow table, you can instead collect the results in memory and keep them as a `DataFrame`.
Then any operations on this in-memory (cached) `DataFrame` are typically _very_ fast.

snippet: reference/dataframe_performance[cache]

## Leverage sparsity to minimize scans
In a write once, read many paradigm adding an additional sparse column can enable cheap access to data of interest via filtering.
The Rerun Data Platform has the ability to "push down" filters to greatly reduce the amount of data returned, improving query performance.
In this example we take advantage of this fact by filtering based on a sparse marker we have intentionally inserted into the recording.

snippet: reference/dataframe_performance[sparsity]
