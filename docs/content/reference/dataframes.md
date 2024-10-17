---
title: Dataframes
order: 300
---

Rerun, at its core, is a database. As such, you can always get your data back in the form of tables (also known as dataframes, or records, or batches...).

This can be achieved in three different ways, depending on your needs:
* using the dataframe API, currently available in [Python](https://ref.rerun.io/docs/python/stable/common/dataframe/) and [Rust](https://docs.rs/crate/rerun/latest),
* using the [blueprint API](../concepts/blueprint) to configure a [dataframe view](types/views/dataframe_view) from code,
* or simply by setting up [dataframe view](types/views/dataframe_view) manually in the UI.

This page is meant as a reference to get you up and running with these different solutions as quickly as possible.
For an in-depth introduction to the dataframe API and the possible workflows it enables, check out [our Getting Started guide](../getting-started/data-out) or one of the accompanying [How-Tos](../howto/dataframe-api).


> We'll need an RRD file to query. Either use one of yours, or grab some of the example ones, e.g.:
> ```
> curl 'https://app.rerun.io/version/latest/examples/dna.rrd' -o - > /tmp/dna.rrd
> ```

### Using the dataframe API

The following snippet demonstrates how to query the first 10 rows in a Rerun recording:

snippet: reference/dataframe_query

Check out the API reference to learn more about all the ways that data can be searched and filtered:
* [🐍 Python API reference](https://ref.rerun.io/docs/python/stable/common/dataframe/)
* [🐍 Python example](https://github.com/rerun-io/rerun/blob/124be04fb1a5695d50813860903f575010bf5bc7/examples/python/dataframe_query/dataframe_query.py)
* [🦀 Rust API reference](https://docs.rs/crate/rerun/latest)
* [🦀 Rust example](https://github.com/rerun-io/rerun/blob/124be04fb1a5695d50813860903f575010bf5bc7/examples/rust/dataframe_query/src/main.rs)


### Using the blueprint API to configure a dataframe view

TODO: incoming.

Check out the blueprint API reference to learn more about all the ways that data can be searched and filtered:
* [🐍 Python blueprint API reference](https://ref.rerun.io/docs/python/latest/common/blueprint_apis/)


### Setting up dataframe view manually in the UI

TODO: incoming.
