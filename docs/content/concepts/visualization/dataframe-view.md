---
title: Dataframe view
order: 500
---

The Viewer can display your data as a table using the dataframe view.
This is useful for inspecting raw values, debugging, or understanding the structure of your logged data.

There are two ways to configure a dataframe view: using the blueprint API from code, or manually in the UI.


## Using the blueprint API

The following snippet demonstrates how to visualize an entire Rerun recording using latest-at (time-aligned) semantics by displaying the results in a [dataframe view](../../reference/types/views/dataframe_view.md):

snippet: reference/dataframe_view_query

<picture>
  <img src="https://static.rerun.io/dataframe_query_example/d3dc908edb09377fbdc4c8f16b1b35a7a35a5e7d/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/dataframe_query_example/d3dc908edb09377fbdc4c8f16b1b35a7a35a5e7d/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/dataframe_query_example/d3dc908edb09377fbdc4c8f16b1b35a7a35a5e7d/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/dataframe_query_example/d3dc908edb09377fbdc4c8f16b1b35a7a35a5e7d/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/dataframe_query_example/d3dc908edb09377fbdc4c8f16b1b35a7a35a5e7d/1200w.png">
</picture>


## Re-using blueprint files across SDKs

While the blueprint APIs are currently only available through Python, blueprints can be saved and re-logged as needed from any language our SDKs support.

First, save the blueprint to a file (`.rbl` by convention) using either the Viewer (**Menu > Save blueprint**) or the Python API:

snippet: reference/dataframe_save_blueprint

Then log that blueprint file in addition to the data itself:

snippet: reference/dataframe_view_query_external

Check out the blueprint API and `log_file_from_path` references to learn more:
- [🐍 Python blueprint API reference](https://ref.rerun.io/docs/python/stable/common/blueprint_apis/)
- [🐍 Python `log_file_from_path`](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.log_file_from_path)
- [🦀 Rust `log_file_from_path`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.log_file_from_path)
- [🌊 C++ `log_file_from_path`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#a20798d7ea74cce5c8174e5cacd0a2c47)

You can learn more about saving and loading blueprints in [Configure the Viewer](../../getting-started/configure-the-viewer.md#save-and-load-blueprint-files).


## Setting up a dataframe view in the UI

The same [dataframe view](../../reference/types/views/dataframe_view.md) shown above can be configured purely from the UI:

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/dataframe/df-dna-demo.webm" type="video/webm" />
</video>
