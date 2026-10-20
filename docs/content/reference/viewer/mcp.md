---
title: MCP server
order: 5
---

The Rerun CLI includes an [MCP](https://modelcontextprotocol.io/) server that lets agents such as Codex or Claude interact with a running Viewer.
It allows the agent to interact with the viewer like a real user, allowing it to interact with the ui, adjust settings, type text, or take screenshots.
It works similar to e.g. Claude for Chrome or Codex Computer Use, but tailored to Rerun.

Some things it is useful for:

- **Debugging a logging script**: "The left camera doesn't show up in the viewer, investigate and fix via the mcp."
- **Adding a custom blueprint**: "Create a blueprint with two tabs: The first is a grid of the cameras, the second shows the map and 3D view. Verify with rerun viewer-mcp."
- **Explore recordings**: "Look at each recording in this dataset and find where it rains. Write a report including screenshots."

To change how the Viewer looks, an agent writes a blueprint with `rerun_set_blueprint` instead of clicking through the Viewer's menus.
This covers a recording's layout and views as well as a table's columns, row filters, and cards.
See [Reading and writing the blueprint](#reading-and-writing-the-blueprint).

## Setup

The server is the `viewer-mcp` subcommand of the `rerun` binary, speaking MCP over stdio.
It connects to a separate, already-running Viewer over gRPC, so an MCP client only needs to know how to launch `rerun viewer-mcp`.

Add it to **Claude Code**:

```sh
claude mcp add rerun -- rerun viewer-mcp
```

Add it to **Codex**:

```sh
codex mcp add rerun -- rerun viewer-mcp
```

Or configure any MCP client manually. Most accept a `mcp.json` config like this:

```json
{
  "mcpServers": {
    "rerun": {
      "command": "rerun",
      "args": ["viewer-mcp"],
      "env": {
        "RUST_LOG": "re_viewer_mcp=info,warn"
      }
    }
  }
}
```

These assume `rerun` is installed on your `PATH` (see [install rerun](../../getting-started/install-rerun.md)).
If it is not, replace `rerun` with the absolute path to the binary.

By default the agent picks the Viewer with the `rerun_connect` tool.
To bind the server to one Viewer up front, pass its gRPC address: `rerun viewer-mcp --endpoint http://127.0.0.1:9876`.
The server then connects on startup, so the agent can skip `rerun_connect`.
If the Viewer is not running yet, the server tells the agent to call `rerun_connect` instead.

To check whether an address is a Rerun Viewer before connecting, use [gRPC server reflection](../grpc.md): `grpcurl -plaintext 127.0.0.1:9876 list` names the services it speaks.

## Headless usage

The MCP server works against a headless Viewer too, which is convenient for agents running in the background, in CI, or on a server without a display.
Ask the agent to launch the Viewer headless or in the background, and it will start it with `rerun --headless`.

## Two families of tools

The tool name says which family it belongs to, and the agent is told to prefer the high-level one.

The `rerun_*` tools (`rerun_get_viewer_state`, `rerun_get_recording_schema`, `rerun_set_time_cursor`, `rerun_open_url`, `rerun_close_recordings`, `rerun_save_screenshot`, …) are high level: each names a Viewer action and carries it out in one call.
They are generated from `viewer_control.proto`, so they cover exactly what that service can do, and they travel on its `ViewerControl` endpoint.

Among them, `rerun_list_commands`, `rerun_describe_commands`, and `rerun_run_command` reach every command of the Viewer's command palette.
Listing returns only the ids by default, so the agent can pick a command without reading the details of every one.
Commands that open a native modal, such as a file dialog, are refused unless the caller opts in, because the modal freezes the Viewer until a person answers it.

The remaining tools (`widget_tree`, `click`, `type_text`, `hover`, `scroll`, `screenshot`, …) are low level: they drive the widgets one input event at a time.
They come from `egui_mcp` and travel on the separate `EguiInspect` endpoint, which carries an opaque body the Viewer's `egui_inspection` plugin services.

They reach anything the Viewer can do, including everything the high-level tools do not cover, but at the cost of several calls and a lot of agent context per action, and they break whenever the layout moves.
That is why an agent should drop to them only when no `rerun_*` tool fits.

## What the agent sees

Besides the accessibility tree and screenshots, the server gives the agent the same signals a user relies on to tell whether the Viewer is healthy:

- **View reports**: `rerun_get_viewer_state` lists every view of the current blueprint together with the warnings and errors it reported the last time it was shown, the same ones behind the warning icon in a view's title bar.
  That tells the agent why a view is empty or looks wrong without a screenshot.
- **Viewer logs**: the Viewer's log messages (INFO and above) since the previous tool call are appended to every tool result, and the `rerun_get_viewer_logs` tool returns the recent history.
  The agent notices the same warnings and errors you see in the notification panel.
- **What is still loading**: `rerun_get_viewer_state` lists the data sources the Viewer is still loading from, the same ones its loading screen spins for.
  `rerun_open_url` waits a few seconds for the load to finish, and says so when it is still loading, so this is how the agent tells a recording that is still arriving from one that arrived empty.
  It matters most while a large import is running, because the Viewer paints no frames then and the low-level tools all stall — this one keeps answering.
- **Where the Viewer is**: `rerun_get_location` reports what the Viewer shows as a [Viewer URL](urls.md), and `rerun_set_location` navigates back to it without loading anything.
  Each recording in `rerun_get_viewer_state` carries its own `location`, so switching recordings needs no clicking either.
- **Connected servers**: `rerun_get_viewer_state` lists the Redap servers in the left panel, with the name, kind, and URL of every dataset and table on each, or the error that kept the catalog from loading.
  So "open a dataset on that server" goes straight to `rerun_open_url`, without clicking through the tree.

## Reading the data

`rerun_get_recording_schema` answers what a recording holds: every entity, the components logged on each, whether a component has a static value, and its Arrow datatype.
It works for any open recording, whatever it was loaded from.
Without it an agent guesses entity paths and component names from an archetype's documentation, and a query built on a wrong guess comes back empty rather than complaining — so the agent ends up describing data it never read.
A recording too large to describe in one answer comes back truncated, with a count of the entities left out; the request narrows to a subtree to read the rest.
`rerun_get_dataset_schema` gives the same description for a dataset on a server, merged over all its segments, without opening any of them.
Without a `dataset_url` it describes the dataset on screen, or the dataset of the segment on screen.
It can also ask for the entity paths alone, which lists a whole recording at a size worth reading and leaves the components for a follow-up call.
For a dataset, entity paths alone are the default when no `entity_path` is given.
These describe the entities in the segments, not the columns of a table, which `rerun_get_table_schema` names.

The transform graph tool described below can return transform values, but for general recording values, use the catalog API.
The Viewer hosts a catalog server, and `rerun_get_viewer_state` reports its address as `catalog_url`, along with each recording's `store_id`.
That `store_id` is a `{kind}:{application_id}:{recording_id}` string, whose application id and recording id are its dataset id and segment id in that catalog.
Both ids may contain a colon, so the application id's are escaped as `\:`: the kind runs to the first colon, the application id to the next unescaped one, and the recording id is the rest.
The server instructions tell the agent to query that server with the Python `rerun.catalog` API for entity paths, components, and values rather than guessing them.
For a recording that is not in the catalog, such as one streamed from an SDK or imported from another file format, the agent is told to read the file directly with `rerun.chunk.RrdReader` instead.

## Inspecting transforms

`rerun_get_transform_graph` returns a JSON snapshot of the direct transform graph at the recording's current timeline and time cursor.
It includes canonical frame names, parent/child edges, source entity paths, sample times, and whether chunks were missing.
Missing chunks mean the graph is incomplete; missing edges then do not prove disconnection.
The tool is also available to the built-in agent panel.

Describe the frame relationships shown here.
Do not guess which entities use those frames or how they appear in the viewer.
A single snapshot cannot tell you whether something moves.
Names alone cannot tell you how frames should connect, and a frame without children is normal.
Only report a problem or suggest a fix if data you inspected or a viewer report supports it.
Otherwise describe what you found, or use your structured question tool to ask the user whether there are specific issues they want diagnosed.

All request fields are optional:

- `store_id`: defaults to the active recording.
- `frame_filter`: `FRAME_FILTER_ALL` (default), `FRAME_FILTER_ENTITY_PATH`, or `FRAME_FILTER_NAMED`.
- `edge_filter`: `EDGE_FILTER_ALL` (default), `EDGE_FILTER_STATIC`, or `EDGE_FILTER_TEMPORAL`.
- `include_transforms`: defaults to `false` for compact connectivity inspection; set it to `true` for affine matrices and pinhole parameters.

Agents should choose the representation before diagnosing connectivity:

1. First query with `frame_filter: "FRAME_FILTER_NAMED"` and `edge_filter: "EDGE_FILTER_ALL"`.
2. If named frames exist, assume they are the user's intended representation and keep inspecting and reporting on named frames only, unless the user indicates otherwise.
3. If no named frames exist, inspect `FRAME_FILTER_ENTITY_PATH` instead.

Entity-path-derived frames (`tf#`) are always implicitly present.
The presence of `tf#/some/entity` does **not** mean `/some/entity` uses that frame: the implicit frame can exist even when the entity is assigned to a named frame via `CoordinateFrame`.
A working named tree plus a separate implicit `tf#` tree is normal: agents should not report this as "two disconnected trees", a broken transform setup, or something the user needs to connect.
Named and entity-path frames are usually not mixed, and mixing them is not recommended, although explicit connections between them are supported.
Robotics users often use named frames because they work like ROS TF.
Inspect `FRAME_FILTER_ALL` only when the user asks for both representations or there is evidence of an intentional mixed-frame setup.
If the intended representation is still unclear, use the agent's structured question tool to clarify with the user; the mere presence of implicit `tf#` frames is not ambiguity.

For transform trees with named frames, there are two separate relationships: `Transform3D` connects frames; `CoordinateFrame` component values assign entities to frames.
This API shows only the first.
Entity assignments are omitted by design, not missing or suspect.
Do not flag uninspected assignments as possible problems or things worth checking.
Analyze assignments separately only when explicitly asked or when a concrete user-reported issue or relevant viewer error calls for it.
Use the recording schema to locate `CoordinateFrame` components, then read their values at the relevant time and relevant blueprint overrides.
Neither schema presence nor `source.entity_path` reveals an assignment: the latter only says where a transform was logged.
`CoordinateFrame` does not join frame trees.

When investigating transform problems, call `rerun_get_viewer_state` to check whether the viewer has a spatial view and inspect its reports for transform-topology errors.
Use those reports as diagnostic evidence; absence of a spatial view or reports does not prove correctness.

If a spatial view does not show the expected named-frame tree, check its target frame in the blueprint: a common cause is that the view still targets the default `tf#/` root.
This can also happen with recordings containing several disconnected named-frame trees.
Choose a root frame suitable for the content the user wants to see and set the view's target accordingly.
Use the agent's structured question tool to let the user choose when the intent is unclear.
A view targeting the wrong tree does not mean the recording's trees need to be connected.

Frame filtering removes edges whose endpoints are excluded, including mixed named/entity-path connections.
Implicit hierarchy identity edges count as static.
Keep `edge_filter: "EDGE_FILTER_ALL"` when investigating connectivity within the chosen representation.
Frame `has_transform` describes participation in the unfiltered latest-at graph, even if its edges are filtered out.

The response echoes the effective options and query time.
To inspect another time, call `rerun_set_time_cursor`, then request another graph; pause playback for repeatable inspection.
If no cursor is set, the query uses latest values and reports `query.mode = "latest"` with a null time.
Temporal timestamps are integer strings to preserve precision.
Matrices are arrays of rows acting on column vectors: `parent_from_child` maps child coordinates into parent coordinates.

Entity-path frames (`tf#/…`) follow the entity hierarchy with implicit identity relationships unless overridden.
Named frames decouple spatial relationships from entity paths, like ROS TF: `CoordinateFrame` associates geometry with a frame, and `Transform3D` connects frames.
Pinholes connect 3D and 2D spaces.
In named transform hierarchies, an image or video entity is associated with a pinhole when its `CoordinateFrame` component matches the pinhole's `child_frame`.
This association comes from the component values, not from matching entity paths or names, and is not included in the frame graph.
The graph excludes per-instance poses and view-specific blueprint overrides.
See [Transforms & Coordinate Frames](../../concepts/logging-and-ingestion/transforms.md) for details.

## Reading and writing the blueprint

`rerun_get_blueprint` with a `recording` target returns a recording's active blueprint as JSON: the layout, what each view shows, its properties, and per-entity overrides.
The target's `store_id` defaults to the active recording.
Without any target it reads the blueprint of what is on screen: the table if one is shown, else the active recording.
A recording on screen without a blueprint of its own gives the views the Viewer adds by itself, so the agent always has a layout to start from.
The JSON mirrors the blueprint store row for row, so it matches what the SDKs log and what the blueprint panel shows:

```json
{
  "/container/4a8c…": { "ContainerBlueprint": { "container_kind": "Horizontal", "contents": ["/view/1f2e…", "/view/9b3d…"] } },
  "/view/1f2e…": { "ViewBlueprint": { "class_identifier": "3D", "space_origin": "/world" } },
  "/view/1f2e…/ViewContents": { "ViewContents": { "query": "+ /world/**" } },
  "/view/1f2e…/ViewContents/overrides/world/points": { "Points3D": { "colors": "#ff0010" } },
  "/viewport": { "ViewportBlueprint": { "root_container": "4a8c…" } }
}
```

`rerun_set_blueprint` replaces the whole blueprint with JSON of the same form, and clears anything it leaves out.
So an agent reads the blueprint, edits it, and sends all of it back.
The write is a normal blueprint edit, so the user can undo it.

### Table blueprints

A table, such as a dataset's segment table or a catalog table, has a blueprint of its own with its row filters, layout, and column settings.
`rerun_get_blueprint` and `rerun_set_blueprint` read and replace it in the same JSON form when given a `table` target, whose `table_url` defaults to the table on screen.
Their answer names the recording or table it used, as `store_id` or `table_url`.
Row filters are SQL expressions such as `"task" ILIKE '%pick%'`, and the `filters` docs of `TableBlueprint` list the forms the Viewer shows as editable filter widgets.
A filter that does not fit the table's columns comes back as an error.
`rerun_get_table_schema` names each column's Arrow field name, header name, and datatype, as the table was last shown.

To hide, show, reorder, or rename columns, or to filter rows, write the table blueprint rather than clicking through the **Columns** menu with the low-level tools.
The server instructions and the description of `rerun_set_blueprint` give the agent this rule and its steps: read the column names with `rerun_get_table_schema`, read the blueprint, write it back, and check it with one screenshot.

For example, to tidy up the segment table of a DROID dataset, the agent first opens the table with `rerun_open_url`.
`rerun_get_table_schema` then lists its columns in display order.
It answers `NOT_FOUND` until the table has been shown once, so call it again if it was opened in the same step.

```json
{"columns": [
  {"name": "rerun_segment_id", "display_name": "Segment id", "datatype": "Utf8"},
  {"name": "recording link", "display_name": "Recording link", "datatype": "Utf8"},
  …
  {"name": "property:episode:current_task", "display_name": "Current task", "datatype": "List(Utf8)"},
  {"name": "property:episode:ext1_cam_serial", "display_name": "Ext1 cam serial", "datatype": "List(Utf8)"},
  {"name": "property:episode:ext2_cam_serial", "display_name": "Ext2 cam serial", "datatype": "List(Utf8)"},
  …
  {"name": "property:episode:success", "display_name": "Success", "datatype": "List(Boolean)"},
  …
]}
```

For a table that was never configured, `rerun_get_blueprint` with `{"table": {}}` gives the blueprint the table shows by default: every column in display order, and the columns it hides.
`rerun_set_blueprint` with `{"table": {}}` and this `json` shows the recording link, task, and success columns first, and hides both external camera serials:

```json
{
  "/table/layouts/table": {
    "TableLayout": {
      "column_order": ["recording link", "property:episode:current_task", "property:episode:success"]
    }
  },
  "/table/layouts/table/columns/property\\:episode\\:ext1_cam_serial": { "TableColumn": { "visible": false } },
  "/table/layouts/table/columns/property\\:episode\\:ext2_cam_serial": { "TableColumn": { "visible": false } }
}
```

`column_order` takes the plain column names from `name`, and puts them first.
The other columns keep their default order and visibility.
In a column's entity path, `:` and spaces are escaped as `\:` and `\ `, which JSON writes as `\\:` and `\\ `.
To show a column again, set `"visible": true` or leave its entry out.
Calling `rerun_get_table_schema` again lists the columns in the new order, hidden ones included.

A preview column needs both `"cell_kind": "Preview"` in its `TableColumn` and a `TableColumnPreview` that lists its views.
Without the cell kind, the Viewer ignores `TableColumnPreview`.
This shows a 3D view of `/my_scene` as a preview of the recording link on each card:

```json
{
  "/table/layouts/cards": {
    "CardLayout": { "link": "recording link", "field_order": ["recording link"] }
  },
  "/table/layouts/cards/fields/recording\\ link": {
    "TableColumn": { "cell_kind": "Preview" },
    "TableColumnPreview": { "views": "/view/0f3c9a52-7d1e-4b8a-9c2f-1e5d6a7b8c9d" }
  },
  "/view/0f3c9a52-7d1e-4b8a-9c2f-1e5d6a7b8c9d": {
    "ViewBlueprint": { "class_identifier": "3D", "space_origin": "/my_scene" }
  },
  "/view/0f3c9a52-7d1e-4b8a-9c2f-1e5d6a7b8c9d/ViewContents": {
    "ViewContents": { "query": "+ /my_scene/**" }
  }
}
```

The user can undo and redo a table blueprint write, as with a recording's blueprint.
To go back to the table's default blueprint, run the `table.reset_blueprint_to_default` command, **Reset to default blueprint** in the command palette, with `rerun_run_command`.

To make a layout the default for everyone who opens the table, build it in Python and register it on the server, as described in [Configure table layouts and segment previews](../../howto/visualization/configure-table-blueprints.md).
The [`rerun-blueprint` skill](https://github.com/rerun-io/rerun/blob/main/skills/rerun-blueprint/SKILL.md) gives agents the same steps.

## Closing recordings

The `rerun_close_recordings` tool removes one, many, or all recordings from the Viewer.
An agent that regenerates a file and reopens it repeatedly otherwise accumulates stale recordings, which makes both `rerun_get_viewer_state` and the recording panel hard to read.
Closing a recording does not touch files on disk, but unsaved blueprint edits to it are lost.
A registered recording also stays in the catalog, so it can still be read over the catalog API and reopened later.
