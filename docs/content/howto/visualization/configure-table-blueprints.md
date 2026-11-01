---
title: Configure table layouts and segment previews
order: 550
description: Use Python table blueprints to customize catalog tables and dataset segment tables
---

Use table blueprints to order, rename, or hide columns, embed segment previews, and display rows as cards in catalog tables and dataset segment tables.

> [!NOTE]
> The `rerun.blueprint.table` API is experimental and may change in future releases.

Table blueprints are separate from [blueprints for individual recordings](build-a-blueprint-programmatically.md), though both are saved as `.rbl` files, and segment previews use the same view classes, such as `Spatial3DView`.

An agent changes the table on screen through the [Viewer MCP server](../../reference/viewer/mcp.md#table-blueprints), which reads and writes the same blueprint as JSON.
It writes the table blueprint rather than clicking through the **Columns** menu.
The [`rerun-blueprint` skill](https://github.com/rerun-io/rerun/blob/main/skills/rerun-blueprint/SKILL.md) covers both the Python API and the MCP tools.

The [runnable Python example](https://github.com/rerun-io/rerun/blob/latest/docs/snippets/all/howto/visualization/configure_table_blueprints.py) downloads 20 recordings from the public [DROID sample dataset](https://huggingface.co/datasets/rerun/droid_sample/tree/main), starts a local catalog server, and opens the Viewer.

## Prerequisites

You'll need the Rerun Python SDK and a running catalog server with an existing table or dataset.
For an introduction to catalog tables, see [Table entries](../../concepts/query-and-transform/catalog-object-model.md#table-entries).
For creating and populating tables from query results, follow [Storing query results in catalog tables](../query-and-transform/catalog-tables.md).

The example used here creates `my_table` with a `recording_uri` for each segment and `uuid` and `current_task` columns copied from DROID's episode properties.
It also adds an `id` row key and a boolean `reviewed` column, used for [editable flags](#editing-columns).
(These are example-specific columns, not required table fields.)

## Configuring a catalog table

### Defining a preview column

Let's start by defining a preview for the `recording_uri` column that we'll use on cards.
This only creates a column configuration — we'll add it to the card layout below, then save and register the blueprint.

snippet: howto/visualization/configure_table_blueprints[preview]

`Column` takes the source column's physical name; `name` overrides its display label.
`column_type` overrides the component the column's values are shown as.
It takes any component class, e.g. `rr.components.Url`, `rr.components.Thumbnail` or `rr.components.Color`.
For the UI of one archetype field, pass its component descriptor, e.g. `rr.TextDocument.descriptor_text()`.

Instead of a component, `column_type` also takes a `PreviewCell`, which is what we use here.
`PreviewCell` renders the referenced recording using one or more views.
You configure these views just as you would in a [recording blueprint](build-a-blueprint-programmatically.md), including their contents and view properties.
Pass the views directly to `PreviewCell`, without the container layout tree (`Horizontal`, `Vertical`, `Tabs`, etc.) used to arrange a recording's viewport.
Multiple views are shown side by side.
Previews only come from a table blueprint, since the Viewer has no option to turn a column into a preview.

### Configuring card and table layouts

A card displays one table row as a tile with a title, selected fields, and optional segment previews.
Cards provide a visual alternative to rows and columns and are often much better suited to show a series of segment previews.

Let's show previews on cards, and put the recording link and episode metadata first in the table:

snippet: howto/visualization/configure_table_blueprints[layouts]

**Card layout:**
- `title`: the column used as the card's heading.
- `link`: the URI column to open when the card is activated.
- `fields`: the columns to display, in order, including any previews; unlisted fields are hidden.

<picture>
  <img src="https://static.rerun.io/table_blueprint_cards/c4375be1b19f3e5ed6a440aa0fceaca50cbf0a6d/full.png" alt="Card layout with the episode UUID as title, a 3D segment preview, and the current task on each card">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/table_blueprint_cards/c4375be1b19f3e5ed6a440aa0fceaca50cbf0a6d/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/table_blueprint_cards/c4375be1b19f3e5ed6a440aa0fceaca50cbf0a6d/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/table_blueprint_cards/c4375be1b19f3e5ed6a440aa0fceaca50cbf0a6d/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/table_blueprint_cards/c4375be1b19f3e5ed6a440aa0fceaca50cbf0a6d/1200w.png">
</picture>

**Table layout:** `TableLayout.columns` puts listed columns first and makes them visible unless `visible=False`; unlisted columns retain their default order and visibility.

<picture>
  <img src="https://static.rerun.io/table_blueprint_table/2c87b46b26c9b9b5d247533b4227255b4facba3f/full.png" alt="Table layout showing the Recording, Uuid, and Current task columns first">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/table_blueprint_table/2c87b46b26c9b9b5d247533b4227255b4facba3f/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/table_blueprint_table/2c87b46b26c9b9b5d247533b4227255b4facba3f/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/table_blueprint_table/2c87b46b26c9b9b5d247533b4227255b4facba3f/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/table_blueprint_table/2c87b46b26c9b9b5d247533b4227255b4facba3f/1200w.png">
</picture>

Column settings are independent between the two layouts: here, the Viewer automatically displays `recording_uri` as a link in the table, while `PreviewCell` turns it into a preview on cards.

Setting `card_layout` enables cards and makes them the initial layout.
`PreviewsConfig` sets the timeline, play state, playback speed, and start point of the previews.
These settings are shared by all preview columns, whether they're in a table or on cards.
Without a `timeline`, the Viewer picks one automatically.
Pass `play_state="playing"` to start all previews playing, and `playback_speed` to change their speed, e.g. `playback_speed=0.5` for half speed.
It defaults to `0.5`, the middle of the recording.
The play and speed buttons in the table toolbar change the same settings, and a single preview can still be played or paused on its own.

### Saving and registering the blueprint

Save the blueprint to an `.rbl` file, then register it on the table (`catalog_url` is your server's URL):

snippet: howto/visualization/configure_table_blueprints[register_table]

`register_blueprint` sets the default unless you pass `set_default=False`.

> [!NOTE]
> Registration does not upload the file: the server must be able to read the URI.
> This local example requires the server to access the same file path.
> For remote servers, see [Using remote storage](#using-remote-storage).

### Editing columns

To make a column editable, configure it with `editable=True` and add it to the layout's `columns` or `fields` list.
A cell can be edited when its component type has an editor in the Viewer.
A column is also editable without a blueprint when its field in the table schema is created with `editable=True`, e.g. `rr.components.Flag.table_field("reviewed", editable=True)`.
See [Give columns semantic meaning](../query-and-transform/catalog-tables.md#give-columns-semantic-meaning).

To edit the boolean `reviewed` column as flag buttons, use `column_type=rr.components.Flag`.
Card layouts show the first flag field in the card header:

snippet: howto/visualization/configure_table_blueprints[flags]

<picture>
  <img src="https://static.rerun.io/table_blueprint_cards_flags/afad289d2beba7827f6586da1fc568dfec2de87e/full.png" alt="Card layout with a clickable flag in each card header">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/table_blueprint_cards_flags/afad289d2beba7827f6586da1fc568dfec2de87e/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/table_blueprint_cards_flags/afad289d2beba7827f6586da1fc568dfec2de87e/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/table_blueprint_cards_flags/afad289d2beba7827f6586da1fc568dfec2de87e/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/table_blueprint_cards_flags/afad289d2beba7827f6586da1fc568dfec2de87e/1200w.png">
</picture>

To edit a string `notes` column as text, use `column_type=rr.components.Text`:

snippet: howto/visualization/configure_table_blueprints[text]

Editing requires write access to a catalog table with a row-key column marked with `rr.SORBET_IS_TABLE_INDEX` metadata.
The Viewer uses this key to persist edits. See [Update a table](../query-and-transform/catalog-tables.md#update-a-table) for details.
Editing is currently not supported on dataset segment tables.

### Filtering rows

`filters` hides the rows that don't match every filter, in both layouts:

snippet: howto/visualization/configure_table_blueprints[filters]

`rrb.table.col` builds filters for one column.
It only builds the filters that can be edited in the Viewer's filter bar:
- numbers: `eq()`, `ne()`, `lt()`, `le()`, `gt()`, `ge()`, or the operators `==`, `!=`, `<`, `<=`, `>`, `>=`
- booleans: `is_true()`, `is_false()`, `is_null()`, `is_not_true()`, `is_not_false()`, `is_not_null()`
- strings: `str.contains()`, `str.does_not_contain()`, `str.starts_with()`, `str.ends_with()`, matched as plain text ignoring case
- timestamps: `before()`, `after()`, `between()`, and relative ranges such as `today()` and `last_week()`, and the negated ranges such as `not_between()` and `not_today()`

The negated filters such as `ne()` and `str.does_not_contain()` also select the rows where the value is null.
For a list column, use the same filters on `.list`, e.g. `col("tags").list.str.contains("pick")`, to select the rows where any value of the list matches.
The negated filters then select the rows where no value matches.

`col` doesn't know the type of the column, so make sure the filter matches the given column.
The Viewer shows a filter for another type, such as `str.contains()` on a number column, as SQL text.
If the SQL doesn't work on the column, the Viewer shows the error on that filter and doesn't apply it.

Instead of using `rrb.table.col`, you can also provide a string with a raw SQL expression in [DataFusion's SQL dialect](https://datafusion.apache.org/user-guide/sql/index.html), with double-quoted column names.
It filters the table like any other filter, and the Viewer shows it as SQL text.

## Configuring a dataset's segment table

Dataset segment tables use the same blueprint API.
The Viewer provides a `"recording link"` column for previews and card links; metadata columns use [property names](../../concepts/query-and-transform/properties-and-segments.md), such as DROID's `"property:episode:uuid"`.
Register the saved blueprint with `segment_table=True`:

snippet: howto/visualization/configure_table_blueprints[register_segments]

Without `segment_table=True`, registration sets the blueprint for opening individual recordings, not the segment table.

## Using remote storage

Upload the `.rbl` file to storage the server has permission to read, then register its URI:

snippet: howto/visualization/configure_table_blueprints[register_remote]

For dataset segment tables, use `get_dataset(…).register_blueprint(uri, segment_table=True)` instead.

## What the blueprint stores

A table blueprint is stored as blueprint entities, which is what an `.rbl` file holds and what the [Viewer MCP server](../../reference/viewer/mcp.md) reads and writes as JSON.
Each Python class maps to these entity paths and archetypes:

| Python | Entity path | Archetypes |
|--------|-------------|------------|
| `TableBlueprint` | `/table` | `TableBlueprint`, `PreviewsConfig` |
| `TableLayout` | `/table/layouts/table` | `TableLayout`, with the columns in `column_order` |
| `Column` in a `TableLayout` | `/table/layouts/table/columns/{column}` | `TableColumn`, and `TableColumnPreview` for a `PreviewCell` |
| `CardLayout` | `/table/layouts/cards` | `CardLayout`, with the fields in `field_order` |
| `Column` in a `CardLayout` | `/table/layouts/cards/fields/{column}` | `TableColumn`, and `TableColumnPreview` for a `PreviewCell` |
| A view in a `PreviewCell` | `/view/{view_id}` | `ViewBlueprint` and its view properties, as in a recording blueprint |

In an entity path, `:` and spaces in a column name are escaped as `\:` and `\ `.
`TableColumnPreview.views` lists the `/view/{view_id}` paths of its views.
A column with a `TableColumnPreview` shows as a preview.
`TableColumn.column_type` holds the component identifier and component type set by `column_type`, e.g. `rerun.components.Flag`.

## Further examples

The runnable snippet above covers the basic workflow.
For more complete examples, including setup and usage instructions, see:

- [Table blueprints](https://github.com/rerun-io/rerun/blob/latest/examples/python/table_blueprints/README.md): multiple preview views (3D, 2D, and plots), editable flags, and registration on local or remote catalogs.
- [Table grid with flags](https://github.com/rerun-io/rerun/blob/latest/examples/python/table_grid_with_flags/README.md): a standalone editable table demonstrating row keys and persisted flag updates, without segment previews.

For more about catalog entries and their blueprints, see the [Catalog object model](../../concepts/query-and-transform/catalog-object-model.md).
For the available configuration options, see the [Python table blueprint API reference](https://ref.rerun.io/docs/python/stable/blueprint_table/).
