---
title: Send entire timeseries at once
order: 0
description: How to use the Rerun SDK to log big chunks of data in one call
---


Sometimes you want to send big chunks of data to Rerun efficiently. To do so, you can use `send_columns`.

`send_columns` lets you efficiently log the state of an entity over time, logging multiple time and component columns in one call.

In contrast to the `log` function, `send_columns` does NOT add any other timelines to the data. Neither the built-in timelines `log_time` and `log_tick`, nor any [user timelines](../../concepts/timelines.md). Only the timelines explicitly included in the call to `send_columns` will be included.

API docs of `send_columns`:
* [🌊 C++](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#ad17571d51185ce2fc2fc2f5c3070ad65)
* [🐍 Python](https://ref.rerun.io/docs/python/stable/common/columnar_api/#rerun.send_columns)
* [🦀 Rust](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.send_columns)


### Using `send_columns` for logging scalars
snippet: archetypes/scalar_send_columns


### Using `send_columns` for logging images
snippet: archetypes/image_send_columns


### Using `send_columns` for logging points
Each row in the component column can be a batch of data, e.g. a batch of positions.
This lets you log the evolution of a point cloud over time efficiently.

snippet: archetypes/points3d_send_columns.py

### Using `send_columns` for logging custom components

An entire batch of a custom component can be logged at once using [`rr.AnyBatchValue`](https://ref.rerun.io/docs/python/0.20.0/common/custom_data/#rerun.AnyBatchValue) along with `send_column`:

snippet: howto/any_batch_value_send_columns

The [`rr.AnyValues`](https://ref.rerun.io/docs/python/0.20.0/common/custom_data/#rerun.AnyValues) class can also be used to log multiple components at a time.
It does not support partitioning, so each component batch and the timeline must hold the same number of elements.

snippet: howto/any_values_send_columns
