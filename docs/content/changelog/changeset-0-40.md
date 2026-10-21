---
title: "0.40"
order: 970
---

## Highlights

### Viewer design refresh

The Viewer has a refreshed home screen, a centered command palette in the top bar, and more consistent typography and colors across light and dark themes.
Rounded surfaces, animated menus and dialogs, and refined list items make navigation and controls feel more consistent throughout the Viewer.
The top bar also has a single toggle for diagnostic metrics.
The macOS menu bar and web menu share commands and shortcuts with the command palette.

See [Configure the Viewer](../getting-started/configure-the-viewer.md) for an introduction to the Viewer.

<picture>
  <img src="https://static.rerun.io/viewer-design-refresh/b790d059288a5c1dca91974b556df497cff78f8c/full.png" alt="The refreshed Viewer showing a robot manipulation episode, with the command palette in the top bar">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viewer-design-refresh/b790d059288a5c1dca91974b556df497cff78f8c/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viewer-design-refresh/b790d059288a5c1dca91974b556df497cff78f8c/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viewer-design-refresh/b790d059288a5c1dca91974b556df497cff78f8c/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viewer-design-refresh/b790d059288a5c1dca91974b556df497cff78f8c/1200w.png">
</picture>

### Inspect transform graphs via MCP

Rerun already provides a [transform debugging UI](../concepts/logging-and-ingestion/transforms.md#debugging-transform-trees) for inspecting frame connections and transform values at the current time cursor.
Agents can now inspect that same transform graph via MCP using `rerun_get_transform_graph`, which returns frame connections and their sources as JSON, with optional filters and numerical transform values to help debug spatial relationships.
See [Inspecting transforms via MCP](../reference/viewer/mcp.md#inspecting-transforms) for details.

## New features

### Experimental: table filters and sorting can be defined in the table blueprint and filters run faster

Table filters can now be defined in the table blueprint. Either using `rrb.table.col` to get filters that the viewer UI can edit, or by passing an SQL expression.

The column a table is sorted by can also be defined in the table blueprint.
Sorting a column in the viewer saves it to the table blueprint, so the sort still applies after you navigate back to the table.

```python
import rerun.blueprint as rrb
from rerun.blueprint.table import col

rrb.TableBlueprint(
    # The blueprint doesn't know the schema of your table, so you have
    # to make sure the filters make sense for each column.
    filters=[
        col("task").str.contains("pick"),
        col("success").is_true(),
        col("created").last_week(),
        col("tags").list.str.does_not_contain("broken"),
        'length("notes") > 0',  # Can also pass SQL expressions.
    ],
    sort_by=rrb.table.SortBy("score", "descending"),
)
```

A side effect of using SQL for filters is that they are also much faster in many cases: about 1000× faster for timestamp columns, and 20–300× faster on list columns.

See [Filtering rows](../howto/visualization/configure-table-blueprints.md#filtering-rows) for the full list of filters.
The `rerun.blueprint.table` API is experimental and may change in future releases.

### Catalog API improvements

- `DatasetEntry.segment()` returns a `Segment` handle to inspect a segment's layers and schema, get its Viewer URL, and unregister a single layer.
  See [managing segment layers](../concepts/query-and-transform/catalog-object-model.md#managing-segment-layers-with-the-python-catalog-api).
- `DatasetEntry.layer(name)` returns a live `Layer` handle whose `.segment_ids()` lists the segments currently containing that layer.
- `DatasetEntry.schema()` and `DatasetSelection.schema()` return catalog schema snapshots, which convert to Arrow with `.to_arrow()`.
  `DatasetEntry.arrow_schema()` is deprecated in favor of `DatasetEntry.schema().to_arrow()`.
- `DatasetEntry.info()` summarizes a dataset (segment count, size, layers, timeline ranges, contents and property columns) without reading any recording data.
  See [getting an overview](../howto/query-and-transform/get-data-out.md#getting-an-overview).

### The selection panel shows which layers a component comes from

Selecting a component of a dataset segment now lists the [layers](../howto/logging-and-ingestion/layers.md) its data comes from.

<picture>
  <img src="https://static.rerun.io/component-layer/9b4d33a5cc99f35fe5ac93cafe95bd6dcbc0f15d/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/component-layer/9b4d33a5cc99f35fe5ac93cafe95bd6dcbc0f15d/480w.png">
</picture>

Data from an asset shows up as the asset's name, and hovering it shows which of the asset's layers the data comes from.

<picture>
  <img src="https://static.rerun.io/component-asset-layer/8a7dd6a1bb5ab9846e526aa550b76dc014cad3d8/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/component-asset-layer/8a7dd6a1bb5ab9846e526aa550b76dc014cad3d8/480w.png">
</picture>

### Edit asset coverage in the viewer

The assets tab of a dataset now shows which segments each asset applies to, and lets you change which segments an asset applies to:

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/ffb69f38b2c182b51bb250ac8d17ee406cec920d_asset_coverage_ui.mp4" type="video/mp4" />
</video>

See [assets](../concepts/query-and-transform/catalog-object-model.md#assets) for more information on the concept of assets.

### Assets without normals are shaded

OBJ, PLY, glTF and DAE [`Asset3D`](../reference/types/archetypes/asset3d.md) assets that have no vertex normals are now lit and shaded: the Viewer computes flat per-triangle normals for them instead of rendering them unlit.
To keep an asset unlit, include all-zero vertex normals in the file.

Before:

<picture>
  <img src="https://static.rerun.io/mesh-smooth-normals-before/14bf42599bcd760342153578d24df2855c7b9771/full.png" alt="The Rerun logo mesh without normals, rendered as a flat, unlit white silhouette">
</picture>

After:

<picture>
  <img src="https://static.rerun.io/mesh-flat-normals-after/36e12e0553ebd4dc6e1f75541df5cdc950312350/full.png" alt="The same mesh shaded with computed flat normals, with its letters clearly visible">
</picture>

### Experimental: LeRobot dataset metadata

`LeRobotReader.metadata()` returns the dataset-wide metadata of a LeRobot dataset as one static chunk at `/__lerobot` (configurable with `entity_path_prefix`), holding the text of `meta/info.json` and, when present, `meta/stats.json`.
The metadata is the same for every episode, so write it to its own `.rrd` and register it once with `DatasetEntry.register_asset()` instead of copying it into each episode.
The dataloader example now reads feature shapes and normalization statistics from this asset.
`LeRobotReader` is part of `rerun.experimental` and may change in future releases.

[Train a policy on a Rerun dataset](../getting-started/train.md)
[Dataloader example](https://github.com/rerun-io/rerun/tree/main/examples/python/dataloader)

## Other

- **Viewer-control operations answer once they are done**: `open_url`, `close_recordings`, `set_time_cursor`, `set_blueprint`, and `run_command` now answer once the Viewer has carried them out, so the next `viewer_state()` sees the result. `open_url` waits a few seconds at most for what it loads, and `ViewerClient.open_url` returns `False` if it was still loading by then. See the [MCP reference](../reference/viewer/mcp.md).
- **Undo and redo table blueprint edits**: Edits to a table's blueprint, such as filters and column visibility, can now be undone and redone.
- **Custom table filters show their SQL with syntax highlighting**: Hovering a custom table filter pill now shows its SQL with syntax highlighting, and the text can be selected and copied.
- **LeRobot v2.1 videos load as `VideoStream`**: [LeRobot](../howto/logging-and-ingestion/lerobot.md) v2.1 datasets now load video as `VideoStream` instead of `AssetVideo`, like v3.0. Video with B-frames, such as most H.264, now needs `ffmpeg` on the `PATH`, and the re-encode can make ingesting these datasets slower.

## Breaking changes

### Python 3.11 is the minimum supported version

Python 3.10 reaches [end-of-life in October 2026](https://devguide.python.org/versions/).
The Rerun SDK now requires Python 3.11 or later, and importing `rerun` on Python 3.10 raises a `RuntimeError`.

To keep using Rerun, upgrade to Python 3.11 or later.
See [what's new in Python 3.11](https://docs.python.org/3/whatsnew/3.11.html) for what an upgrade involves.
If you must stay on Python 3.10, pin `rerun-sdk<0.40`.

The [supported Python versions table](https://ref.rerun.io/docs/python/main/common#supported-python-versions) lists which Rerun release works with which Python version.

### `DatasetView` renamed to `DatasetSelection`

`rerun.catalog.DatasetView`, returned by `filter_segments()` and `filter_contents()`, is now `rerun.catalog.DatasetSelection`, with unchanged behavior.
The old name still works but emits a `DeprecationWarning`; use the new name in type hints and `isinstance` checks.

### Rust: catalog methods moved from `re_redap_client` to `re_catalog_client`

High-level catalog methods moved from `re_redap_client` to `CatalogClient` in the new `re_catalog_client` crate.

---

Looking for an older release? See the [migration guides for 0.33 and earlier](../reference/migration.md).
