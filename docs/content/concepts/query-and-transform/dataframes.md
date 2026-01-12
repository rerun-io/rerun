---
title: Dataframes
order: 100
---

Rerun, at its core, is a database. As such, you can always get your data back in the form of tables (also known as dataframes, or records, or batches...).

This can be achieved in three different ways, depending on your needs:
* using the dataframe API, currently available in [Python](https://ref.rerun.io/docs/python/stable/common/dataframe/) and [Rust](https://docs.rs/rerun/latest/rerun/dataframe/index.html),
* using the [blueprint API](../visualization/blueprints.md) to configure a [dataframe view](../../reference/types/views/dataframe_view.md) from code,
* or simply by setting up [dataframe view](../../reference/types/views/dataframe_view.md) manually in the UI.

This page is meant as a reference to get you up and running with these different solutions as quickly as possible.
For an in-depth introduction to the dataframe API and the possible workflows it enables, check out [our Getting Started guide](../../getting-started/data-out) or one of the accompanying [How-Tos](../../howto/query-and-transform/get-data-out.md).


> We'll need an RRD file to query. Either use one of yours, or grab some of the example ones, e.g.:
> ```
> curl 'https://app.rerun.io/version/latest/examples/dna.rrd' -o - > /tmp/dna.rrd
> ```

### Using the dataframe API

The following snippet demonstrates how to query the first 10 rows in a Rerun recording using latest-at (i.e. time-aligned) semantics:

snippet: reference/dataframe_query

Check out the API reference to learn more about all the ways that data can be searched and filtered:
* [🐍 Python API reference](https://ref.rerun.io/docs/python/stable/common/dataframe/)
  * [Example](https://github.com/rerun-io/rerun/blob/c00a9f649fd4463f91620e8e2eac11355b245ac5/examples/python/dataframe_query/dataframe_query.py)
* [🦀 Rust API reference](https://docs.rs/rerun/latest/rerun/dataframe/index.html)
  * [Example](https://github.com/rerun-io/rerun/blob/c00a9f649fd4463f91620e8e2eac11355b245ac5/examples/rust/dataframe_query/src/main.rs)


### Using the blueprint API to configure a dataframe view

The following snippet demonstrates how visualize an entire Rerun recording using latest-at (i.e. time-aligned) semantics by displaying the results in a [dataframe view](../../reference/types/views/dataframe_view.md):

snippet: reference/dataframe_view_query

<picture>
  <img src="https://static.rerun.io/dataframe_query_example/d3dc908edb09377fbdc4c8f16b1b35a7a35a5e7d/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/dataframe_query_example/d3dc908edb09377fbdc4c8f16b1b35a7a35a5e7d/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/dataframe_query_example/d3dc908edb09377fbdc4c8f16b1b35a7a35a5e7d/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/dataframe_query_example/d3dc908edb09377fbdc4c8f16b1b35a7a35a5e7d/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/dataframe_query_example/d3dc908edb09377fbdc4c8f16b1b35a7a35a5e7d/1200w.png">
</picture>

#### Aside: re-using blueprint files from other SDKs

While the blueprint APIs are currently only available through Python, blueprints can be saved and re-logged as needed from any language our SDKs support.

First, save the blueprint to a file (`.rbl` by convention) using either the viewer (`Menu > Save blueprint`) or the python API:

snippet: reference/dataframe_save_blueprint

Then log that blueprint file in addition to the data itself:

snippet: reference/dataframe_view_query_external

Check out the blueprint API and `log_file_from_path` references to learn more:
* [🐍 Python blueprint API reference](https://ref.rerun.io/docs/python/stable/common/blueprint_apis/)
* [🐍 Python `log_file_from_path`](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.log_file_from_path)
* [🦀 Rust `log_file_from_path`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.log_file_from_path)
* [🌊 C++ `log_file_from_path`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#a20798d7ea74cce5c8174e5cacd0a2c47)

You can learn more about saving and loading blueprints in [Configure the Viewer](../../getting-started/configure-the-viewer.md#save-and-load-blueprint-files).


### Setting up dataframe view manually in the UI

The same [dataframe view](../../reference/types/views/dataframe_view.md) shown above can be configured purely from the UI:

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/dataframe/df-dna-demo.webm" type="video/webm" />
</video>
