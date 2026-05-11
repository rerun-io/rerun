<!--[metadata]
title = "Table with editable flags"
tags = ["Tables", "Server"]
include_in_manifest = false
-->

## Table grid with flags

Starts a local server with a table containing an index column and a boolean flag column.
The flag column is marked with Arrow metadata so the viewer's card/grid view can toggle flags
and persist them back to the server.

The flag column remains part of the table data. Its current boolean value controls the flag icon shown on each grid card. Clicking the icon immediately updates the visible table state and sends an upsert back to the server containing the row's table-index value plus the new flag value. The `rerun:is_table_index` column is required so the server knows which row to update.

<!-- TODO(#12745): this is still experimental -->
Enable `Settings > Experimental > Table cards and blueprints` in the viewer, then open the
printed URL.

## Run the code

```bash
pip install -e examples/python/table_grid_with_flags
table_grid_with_flags
```

or via pixi/uv:

```bash
pixi run py-build && pixi run uv run examples/python/table_grid_with_flags/table_grid_with_flags.py
```
