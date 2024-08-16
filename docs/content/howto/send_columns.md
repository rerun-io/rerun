---
title: Efficiently log time series data using `send_columns`
order: 800
description: How to use the Rerun SDK to log big chunks of data in one call
---


Sometimes you want to send big chunks of data to Rerun efficiently. To do so, you can use `send_columns`.

`send_columns` lets you efficiently log the state of an entity over time, logging multiple time and component columns in one call.

In contrast to the `log` function, `send_columns` does NOT add any other timelines to the data. Neither the built-in timelines `log_time` and `log_tick`, nor any [user timelines](docs/content/concepts/timelines.md). Only the timelines explicitly included in the call to `send_columns` will be included.


### Using `send_columns` for logging scalars
snippet: archetypes/scalar_send_columns


### Using `send_columns` for logging images
snippet: archetypes/image_send_columns
