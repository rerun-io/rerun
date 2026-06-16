<!--[metadata]
title = "Table blueprints"
tags = ["Tables", "Blueprints", "Server"]
include_in_manifest = false
-->

## Table blueprints

Creates tables whose rows link to recording segment URIs.
The viewer can load those recordings on demand and render row previews with a registered `.rbl` table blueprint.

The example also adds a boolean `marker_flag` column and names it in the table blueprint.
That column is the per-row flag state: the Viewer renders it as a clickable flag on each grid card, updates the visible table immediately when toggled, and upserts the changed boolean value back to the server using the `rerun:is_table_index` column as the row key.
The column is still regular table data, so its saved values are what you get back when you query the table later.

Blueprints can also be registered on a dataset's **own segment table** instead of on a separate demo table, using `DatasetEntry.register_blueprint(..., segment_table=True)`. <!-- NOLINT -->
Use `--target` to choose (see [Run the code](#run-the-code)):

- `tables`: create the demo tables, each with its own table blueprint.
- `dataset`: register a blueprint on the dataset's segment table (no tables created).
- `both`: do both.

The segment-table blueprint leaves `segment_preview_column` and `flag_column` unset — the viewer auto-picks the column to preview, and segment tables have no demo flag column.
Flagging does **not** yet work on dataset segment tables: segment tables have no write operations yet, so flag changes cannot be persisted back to the server. Flagging therefore only works on the demo tables created with `--target tables`.

<!-- TODO(#12746): this is still experimental -->
Table cards and blueprints are experimental.
Enable `Settings > Experimental > Table cards and blueprints` in the viewer.

## Dataset-specific setup

This sample contains a small `Dataset-specific customization` section near the top of `table_blueprints.py`.
Please edit these functions before using it with your own data — the defaults are geared towards RRDs from the DROID dataset and assume that segment-table schema, timeline, entity paths, coordinate frame, and card-title column:

- `extract_dataset_property_columns` — which segment-table columns get copied into the demo tables.
- `setup_preview_views` — all views (plot, 3D, 2D) shared by the table and segment-table blueprints. Any view type can be used for previews!
- `make_dataset_blueprints` — the table blueprints (preview/flag/card-title columns, timeline).
- `make_segment_table_blueprint` — the blueprint registered on the dataset's own segment table (views, timeline).

## Run the code

The sample has two run modes.

### Local server mode

Without `--url`, the script starts a temporary local Rerun server, serves a directory of `.rrd` files
as a dataset named `local`, writes the `.rbl` blueprint files, and (depending on `--target`) creates
the demo tables and registers their blueprints with
`TableEntry.register_blueprint(...)` and/or registers a blueprint on the dataset's segment table with `DatasetEntry.register_blueprint(..., segment_table=True)`. <!-- NOLINT -->

Run without arguments to serve the checked-in sample files from `tests/assets/rrd/sample_5`:

```bash
pip install -e examples/python/table_blueprints
table_blueprints
```

Or pass any dataset directory containing `.rrd` files:

```bash
table_blueprints /path/to/dataset
```

Choose what the blueprints apply to with `--target` (`tables` by default):

```bash
table_blueprints --target dataset   # only the dataset's segment table
table_blueprints --target both      # demo tables and the segment table
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

With `--url`, the script connects as a client to an existing Rerun server or catalog and looks up the dataset by name.
The generated `.rbl` files must be visible to that server before registration.
Use `--write-blueprints-only` to write them locally (both the table blueprints and `segment_table.rbl`), upload them yourself, then rerun with `--blueprint-uri-base` pointing at the uploaded directory.
`--target` works the same way in remote mode.

```bash
table_blueprints --write-blueprints-only --blueprint-dir /tmp/table-blueprints
# Upload /tmp/table-blueprints/*.rbl to a server-visible location, for example s3://my-bucket/table-blueprints/
table_blueprints <dataset-name> --url rerun+https://… --blueprint-dir /tmp/table-blueprints --blueprint-uri-base s3://my-bucket/table-blueprints/
# …or register on the dataset's segment table instead:
table_blueprints <dataset-name> --url rerun+https://… --target dataset --blueprint-dir /tmp/table-blueprints --blueprint-uri-base s3://my-bucket/table-blueprints/
```
