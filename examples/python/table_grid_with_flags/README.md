<!--[metadata]
title = "Table with editable flags"
tags = ["Tables", "Server"]
include_in_manifest = false
-->

## Table grid with flags

Starts a local server with a table containing an index column and a boolean flag column.
The flag column is marked with Arrow metadata so the Viewer's grid view can toggle flags and persist them back to the server.

<!-- TODO(#12745): this is still experimental -->
Enable `Settings > Experimental > Grid view` in the viewer, then open the printed URL.

## Run the code

```bash
pip install -e examples/python/table_grid_with_flags
table_grid_with_flags
```

or via pixi/uv:

```bash
pixi run py-build && pixi run uv run examples/python/table_grid_with_flags/table_grid_with_flags.py
```
