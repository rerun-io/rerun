---
title: Dataframes
order: 300
---

Rerun, at its core, is a database. As such, you can always get your data back in the form of tables (also known as dataframes, or records, or batches...).

This can be achieved in three different ways, depending on your needs:
* using the dataframe API, currently available in [Python](TODO) and [Rust](TODO),
* using the [blueprint API]() to configure a [dataframe view](TODO) from code,
* or simply by setting up [dataframe view](TODO) manually in the UI.

This page is meant as a reference to get you up and running with these different solutions as quickly as possible.
For an in-depth introduction to the dataframe API and the possible workflows it enables, check out [our Getting Started guide](TODO) or one of the accompanying [How-Tos](TODO).


> We'll need an RRD file to query. Either use one of yours, or grab some of the example ones, e.g.:
> ```
> curl 'https://app.rerun.io/version/latest/examples/dna.rrd' -o - > /tmp/dna.rrd
> ```

### Using the dataframe API

The following snippet demonstrates how to query the first 10 rows in a Rerun recording:

snippet: reference/dataframe_query

Check out the API reference to learn more about all the ways that data can be searched and filtered:
* [🐍 Python API reference](TODO)
* [🦀 Rust API reference](TODO)


### Using the blueprint API to configure a dataframe view

The following snippet demonstrates how to setup a dataframe view that queries and displays the first 10 rows in a Rerun recording:

snippet: reference/dataframe_view_query


Check out the blueprint API reference to learn more about all the ways that data can be searched and filtered:
* [🐍 Python blueprint API reference](TODO)


### Setting up dataframe view manually in the UI

TODO:
* video/screenshot/gif/something that demonstrates how to build the same thing manually
