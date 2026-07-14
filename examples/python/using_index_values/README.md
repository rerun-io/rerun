<!--[metadata]
title = "Using index values"
tags = ["DataFrame", "Server",]
channel = "main"
include_in_manifest = false
-->

## Querying at specific index values

This example demonstrates how to use the `using_index_values` parameter to query
a dataset at specific timestamps (or other index values). When you pass index
values directly, only segments whose time range covers the requested values will
return data -- segments that don't overlap are automatically excluded.

Combined with `fill_latest_at=True`, this is useful for sampling data at specific
points in time, such as evaluating the state of all recordings at a fixed set of
timestamps.

### Setup

This example will launch the OSS server which will run on `localhost` with a random port.

### Running

Run the following commands

```bash
pip install -e examples/python/using_index_values
python examples/python/using_index_values/using_index_values.py
```

or to run it via pixi/uv

```bash
pixi run py-build && pixi run uv run examples/python/using_index_values/using_index_values.py
```
