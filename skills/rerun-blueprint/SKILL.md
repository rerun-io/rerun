---
name: rerun-blueprint
description: Change how the Rerun viewer looks by writing a blueprint, for a recording or for a table. Read this whenever asked to change the look of the viewer, such as laying out a recording or dataset, designing a default blueprint, deciding which views show which entities, or making a segment or catalog table look nice by hiding columns, changing column order or filtering rows. Covers archetype-to-view mapping, layout reasoning, the rrb construction API, the contents grammar, table blueprints, and the screenshot loop.
user_invocable: true
allowed-tools: Read, Grep, Bash, WebFetch
---

# Rerun blueprint

**To change how the viewer looks, change the blueprint.** This holds for a
recording's layout, views and their properties, and for a table's columns, row
filters and cards. Don't click through the viewer's menus to do it. With the
viewer MCP, read and write the blueprint with `rerun_get_blueprint` and
`rerun_set_blueprint` rather than the low-level click tools.

A blueprint decides how a recording is shown, and the recording decides what exists.
Read the data, design a first layout, then **iterate from screenshots** until it
reads at a glance. The constructors are discoverable (`dir(rrb)`,
`help(rrb.Spatial3DView)`); this skill covers what you can't guess. Import as
`import rerun.blueprint as rrb`.

## Tables

A dataset's segment table and catalog tables have a table blueprint of their
own, separate from a recording's. It sets which columns show, their order and
names, the row filters, and whether rows show as a table or as cards. When asked
to make a table look nice, hide columns or filter rows, write this blueprint.

This hides two columns and shows three others first. Listed columns come first,
the rest keep their default order and visibility:

```python
blueprint = rrb.TableBlueprint(
    table_layout=rrb.table.TableLayout(
        columns=[
            rrb.table.Column("recording link"),
            rrb.table.Column("property:episode:current_task", name="Task"),
            rrb.table.Column("property:episode:success"),
            rrb.table.Column("property:episode:ext1_cam_serial", visible=False),
            rrb.table.Column("property:episode:ext2_cam_serial", visible=False),
        ],
    ),
)
blueprint.save("my_app", path)
dataset.register_blueprint(path.as_uri(), segment_table=True)
```

`Column` takes the column's name as the table has it, such as
`property:episode:uuid`, not its header. Registering makes it the default for
everyone who opens the table, and the server must be able to read `path`.

Row filters are SQL expressions, one per string, all of which must be true for a
row: `rrb.TableBlueprint(filters=["\"task\" ILIKE '%pick%'", '"duration" > 10'])`.
Double quotes for column names, single quotes for strings, and
`any_match("column", x -> …)` for list columns.

With the viewer MCP, change the table on screen directly:
`rerun_get_table_schema` names the columns, `rerun_get_blueprint` with
`{"table": {}}` reads the table blueprint, and `rerun_set_blueprint` with
`{"table": {}}` replaces it. `rerun_set_blueprint` returns an error for a filter
that fails. A column's entry sits at `/table/layouts/table/columns/<column name>`,
with each `:` and space in the name escaped as `\:` and `\ `, which JSON writes
as `\\:` and `\\ `. The [MCP docs](https://rerun.io/docs/reference/viewer/mcp#table-blueprints)
have a worked example of the JSON. For the whole Python API, including cards
and segment previews, see
[Configure table layouts and segment previews](https://rerun.io/docs/howto/visualization/configure-table-blueprints).

## 1. Read the data

Enumerate every `(entity_path, archetype)` pair first — the archetype picks the
view, the entity path scopes it. For a dataset (see `rerun-catalog-queries`):

```python
for c in dataset.schema().component_columns():
    print(c.entity_path, c.archetype, c.component_name)
```

For a local recording, stream it with `RrdReader` and read `entity_path` and
the archetype off each chunk (see `rerun-chunk-processing`).

## 2. Map archetype to view

| Archetype family                                                                      | View                                                |
| ------------------------------------------------------------------------------------- | --------------------------------------------------- |
| `Points2D`, `Image`, `EncodedImage`, `Boxes2D`, `LineStrips2D`, `Pinhole` projections | `Spatial2DView`                                     |
| `Points3D`, `Mesh3D`, `Boxes3D`, `LineStrips3D`, `Transform3D`, `Asset3D`             | `Spatial3DView`                                     |
| `Scalars`, `SeriesLines`, `SeriesPoints`                                              | `TimeSeriesView`                                    |
| `TextLog` / `TextDocument` / `BarChart`                                               | `TextLogView` / `TextDocumentView` / `BarChartView` |
| `Tensor`, `DepthImage` (heatmap)                                                      | `TensorView`                                        |
| `GeoPoints`, `GeoLineStrings`                                                         | `MapView`                                           |
| `GraphNodes`, `GraphEdges`                                                            | `GraphView`                                         |
| tabular / catalog data                                                                | `DataframeView`                                     |

## 3. Group entities into views

This step is just deciding which entities share a view. Sizing and placement are
mechanics you tune by eye in the screenshot loop, not rules to settle up front.

- **Group by shared path prefix.** Entities under a common prefix usually belong
  in one view. Don't give every entity its own pane: dozens of raw message
  entities go in one `DataframeView`, or get left out, rather than a pane each.
- **One spatial frame per spatial view.** A whole `Transform3D` tree goes in one
  `Spatial3DView` at the common ancestor; a `Pinhole` camera gets its own
  `Spatial2DView` rooted at the camera so images inherit the projection.
- **Collapse scalars.** Many `Scalars` under a prefix → one `TimeSeriesView` over
  the prefix, not one each. Split only when value ranges clash.
- **`column_shares`/`row_shares` are relative weights** on a container's
  children, equal by default. Tune them from the screenshots.

## 4. Construct it

Containers (`Grid`, `Horizontal`, `Vertical`, `Tabs`) hold views. Default a flat
set to `Grid`; use the others for a deliberate split, `Tabs` for alternatives
competing for one slot (left/right/depth cameras). **Always set an explicit
`origin` and `name`** — `origin` defaults to `/`, which dumps the whole tree into
one view (the usual cause of an unreadable blob).

```python
blueprint = rrb.Blueprint(
    rrb.Horizontal(
        rrb.Spatial3DView(origin="/world", name="Scene"),
        rrb.Vertical(
            rrb.Spatial2DView(origin="/world/camera", name="Camera"),
            rrb.TimeSeriesView(origin="/sensors", name="Sensors"),
        ),
        column_shares=[3, 2],
    ),
    rrb.TextLogView(origin="/logs", name="Logs"),
    collapse_panels=True,
)
```

`contents` defaults to `"$origin/**"`. Scope a view with include/exclude rules,
e.g. `contents=["+ $origin/**", "- $origin/internal/**"]`. A bare line is an
include; `/**` is the only wildcard (matches a subtree). Most-specific rule wins,
ties go to the last, unmatched paths are excluded.

**Coordinate frames.** A spatial view only renders entities it can place relative
to its target frame. `Transform3D`/`Pinhole` logged on entities compose down the
tree, so `origin` is enough. But **named frames** (`CoordinateFrame(frame=...)`,
common in ROS/MCAP) live in a separate frame graph — point the view at a frame
the data occupies via `spatial_information=rrb.SpatialInformation(target_frame="<frame>")`.
The tell is an empty 3D view with "No transform path from `<frame>`…" errors:
`origin="/"` targets the root `tf#/`, which connects to nothing if the tf tree
was never materialized. Read the `CoordinateFrame:frame` values, target the one
the main 3D content sits in, and exclude entities in unconnected frames.

## 5. Iterate from screenshots

Spawn the viewer and load the recording once, then re-send blueprints into the
same viewer; each send + screenshot is one cheap iteration. Render, look, revise.

The blueprint binds to the data by store identity, **both application id and
recording id**. A loaded `.rrd` keeps its own identity, so build the stream from
the rrd's values (set both; `recording_id` otherwise defaults to a random one).
Mismatch either and the viewer keeps blueprint and data as separate recordings
and never applies the blueprint.

```python
import time
import rerun as rr
import rerun.blueprint as rrb
from rerun.chunk import RrdReader
from rerun.experimental import ViewerClient

store = RrdReader("segment.rrd").recordings()[0]  # the rrd's application id + recording id

with ViewerClient.spawn(headless=True, port=9879) as viewer:
    rec = rr.RecordingStream(store.application_id, recording_id=store.recording_id)
    rec.connect_grpc(url=viewer.url)
    rec.log_file_from_path("segment.rrd")

    def shot(blueprint, path):
        rec.send_blueprint(blueprint, make_active=True, make_default=True)
        time.sleep(2)  # let the import finish and a frame render; bump if a view is blank
        viewer.save_screenshot(path)  # view_id=view.id for a single view

    shot(blueprint_v1, "bp_v1.png")
    # Read bp_v1.png, revise, send the next one into the same viewer.
    shot(blueprint_v2, "bp_v2.png")
```

To bake a finished blueprint in instead, pass `default_blueprint=` to `rr.init` /
`spawn` / `connect_grpc` / `save`, or `blueprint=` to `notebook_show`.

## Gotchas

- A `DataframeView` shows "Unknown timeline" without a query:
  `query=rrb.archetypes.DataframeQuery(timeline="<timeline>", apply_latest_at=True)`.
- A view back blank? The importer may not have finished (bump the settle) or the
  cursor sits before the data (add `rrb.TimePanel(play_state=rrb.components.PlayState.Following)`).
- `rrb` views and `ViewerClient` (`rerun.experimental`) are unstable. Check
  `help()` if a constructor argument is rejected.

## See also

- `rerun-data-model` — entities, archetypes, timelines.
- `rerun-catalog-queries` — enumerate entities in a dataset.
