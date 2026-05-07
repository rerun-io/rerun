<!--[metadata]
title = "Table blueprints"
tags = ["Tables", "Blueprints", "Server"]
include_in_manifest = false
-->

## Table blueprints

Creates tables with embedded blueprint metadata. Each row links to a recording segment URI; the viewer can load those recordings on demand and render row previews with the embedded blueprint views.

<!-- TODO(#12746): this is still experimental -->
Table blueprints are experimental. Enable `Settings > Experimental > Table blueprints` in the viewer.

## Dataset-specific setup

This sample contains a small `Dataset-specific customization` section near the top of `table_blueprints.py`. Please edit `extract_dataset_property_columns` and `make_dataset_blueprints` before using it with your own data: the defaults are geared towards RRDs from the DROID dataset and assume that segment-table schema, timeline, entity paths, coordinate frame, and card-title column.

## Run the code

The sample has two run modes:

### Local server mode

Without `--url`, the script starts a temporary local Rerun server, serves a directory of `.rrd` files as a dataset named `local`, creates the demo tables in that local server, and keeps running until you press Enter.

Run without arguments to serve the checked-in sample files from `tests/assets/rrd/sample_5`:

```bash
pip install -e examples/python/table_blueprints
table_blueprints
```

Or pass any dataset directory containing `.rrd` files:

```bash
table_blueprints /path/to/dataset
```

Use `--port` if you want the local server to listen on a specific port:

```bash
table_blueprints /path/to/dataset --port 9876
```

Via pixi/uv:

```bash
pixi run py-build && pixi run uv run examples/python/table_blueprints/table_blueprints.py /path/to/dataset
```

### Remote client mode

With `--url`, the script does not start a server. Instead, it connects as a client to an existing Rerun server or catalog, looks up the dataset by name, and creates the demo tables there.

```bash
table_blueprints <dataset-name> --url rerun+https://…
```
