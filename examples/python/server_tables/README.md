<!--[metadata]
title = "Server tables"
tags = ["DataFrame", "Tables", "Server", "Cloud",]
thumbnail = "https://static.rerun.io/server_tables/d5155346d84caed5c53de507708c780727c075ef/480w.png"
thumbnail_dimensions = [480, 358]
-->

## Writing server tables example

The purpose of this example is to demonstrate how one would set up a data flow where you are incrementally
processing partitions within a dataset. The general concept is that you have two tables that you will use,
one for status and one for results. The purpose of the status table is to have a small table that is easy
to query for partitions that have not yet been processed.

In this example, we first create these two tables. Then we collect the available partitions and compare them
to the status table. To demonstrate how you could batch process a portion of your available data, we simply
take a subset of the returned values that are not yet processed. In customer work flows, you will likely
want to pass all available partitions to work, or you might prefer to send off a single partition at
a time. The details of how you select which partitions to process are up to the individual workflows.

The code in this example produces a few lines of status output and then displays both the results
and status tables.

### Setup

This example will launch the OSS server which will run on `localhost` with a random port.

The example will also create a temporary directory. It will not persist after this script has been executed,
so you will need to restart your server if you want to run the example multiple times. If you prefer
to persist the created table, you can change the remove the `with tempfile.TemporaryDirectory()` line and
instead set a specific location for your files.

### Running

Run the following commands

```bash
pip install -e examples/python/server_tables
python examples/python/server_tables/server_tables.py
```

or to run it via pixi/uv

```bash
pixi run py-build && pixi run uv run examples/python/server_tables/server_tables.py
```
