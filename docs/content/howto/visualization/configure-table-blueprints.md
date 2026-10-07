---
title: Configure table layouts and recording previews
order: 550
description: Use Python table blueprints to customize catalog tables and dataset segment tables
---

Use table blueprints to order, rename, or hide columns, embed recording previews, and display rows as cards in catalog tables and dataset segment tables.

> [!NOTE]
> The `rerun.blueprint.table` API is experimental and may change in future releases.

Table blueprints are separate from [blueprints for individual recordings](build-a-blueprint-programmatically.md), though both are saved as `.rbl` files, and recording previews use the same view classes, such as `Spatial3DView`.

The [runnable Python example](https://github.com/rerun-io/rerun/blob/latest/docs/snippets/all/howto/visualization/configure_table_blueprints.py?speculative-link) downloads 20 recordings from the public [DROID sample dataset](https://huggingface.co/datasets/rerun/droid_sample/tree/main), starts a local catalog server, and opens the Viewer.

## Prerequisites

You'll need the Rerun Python SDK and a running catalog server with an existing table or dataset.
For an introduction to catalog tables, see [Table entries](../../concepts/query-and-transform/catalog-object-model.md#table-entries).
For creating and populating tables from query results, follow [Storing query results in catalog tables](../query-and-transform/catalog-tables.md).

The example used here creates `my_table` with a `recording_uri` for each segment and `uuid` and `current_task` columns copied from DROID's episode properties.
It also adds an `id` row key and a boolean `reviewed` column, used for [editable flags](#adding-editable-flags).
(These are example-specific columns, not required table fields.)

## Configuring a catalog table

### Defining a preview column

Let's start by defining a preview for the `recording_uri` column that we'll use on cards.
This only creates a column configuration — we'll add it to the card layout below, then save and register the blueprint.

snippet: howto/visualization/configure_table_blueprints[preview]

`Column` takes the source column's physical name; `name` overrides its display label.

`PreviewCell` is what renders the referenced recording using one or more views.
You configure these views just as you would in a [recording blueprint](build-a-blueprint-programmatically.md), including their contents and view properties.
Pass the views directly to `PreviewCell`, without the container layout tree (`Horizontal`, `Vertical`, `Tabs`, etc.) used to arrange a recording's viewport.
Multiple views are shown side by side.

### Configuring card and table layouts

A card displays one table row as a tile with a title, selected fields, and optional recording previews.
Cards provide a visual alternative to rows and columns and are often much better suited to show a series of recording previews.

Let's show previews on cards, and put the recording link and episode metadata first in the table:

snippet: howto/visualization/configure_table_blueprints[layouts]

**Card layout:**
- `title`: the column used as the card's heading.
- `link`: the URI column to open when the card is activated.
- `fields`: the columns to display, in order, including any previews; unlisted fields are hidden.

<picture>
  <img src="https://static.rerun.io/table_blueprint_cards/c4375be1b19f3e5ed6a440aa0fceaca50cbf0a6d/full.png" alt="Card layout with the episode UUID as title, a 3D recording preview, and the current task on each card">
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
`PreviewsConfig` selects a timeline for the previews (omit it to let the Viewer choose automatically).
This setting is shared by all preview columns, whether they're in a table or on cards.

### Saving and registering the blueprint

Save the blueprint to an `.rbl` file, then register it on the table (`catalog_url` is your server's URL):

snippet: howto/visualization/configure_table_blueprints[register_table]

`register_blueprint` sets the default unless you pass `set_default=False`.

> [!NOTE]
> Registration does not upload the file: the server must be able to read the URI.
> This local example requires the server to access the same file path.
> For remote servers, see [Using remote storage](#using-remote-storage).

### Adding editable flags

To make an existing boolean `reviewed` column editable, configure it with `editable=True` and a `FlagCell`, then add it to the layout's `columns` or `fields` list:

snippet: howto/visualization/configure_table_blueprints[flags]

<picture>
  <img src="https://static.rerun.io/table_blueprint_cards_flags/afad289d2beba7827f6586da1fc568dfec2de87e/full.png" alt="Card layout with a clickable flag in each card header">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/table_blueprint_cards_flags/afad289d2beba7827f6586da1fc568dfec2de87e/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/table_blueprint_cards_flags/afad289d2beba7827f6586da1fc568dfec2de87e/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/table_blueprint_cards_flags/afad289d2beba7827f6586da1fc568dfec2de87e/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/table_blueprint_cards_flags/afad289d2beba7827f6586da1fc568dfec2de87e/1200w.png">
</picture>

Editing requires write access to a catalog table with a row-key column marked with `rr.SORBET_IS_TABLE_INDEX` metadata.
The Viewer uses this key to persist edits. See [Update a table](../query-and-transform/catalog-tables.md#update-a-table) for details.
Editing flags is currently not supported on dataset segment tables.

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

## Further examples

The runnable snippet above covers the basic workflow.
For more complete examples, including setup and usage instructions, see:

- [Table blueprints](https://github.com/rerun-io/rerun/blob/latest/examples/python/table_blueprints/README.md): multiple preview views (3D, 2D, and plots), editable flags, and registration on local or remote catalogs.
- [Table grid with flags](https://github.com/rerun-io/rerun/blob/latest/examples/python/table_grid_with_flags/README.md): a standalone editable table demonstrating row keys and persisted flag updates, without recording previews.

For more about catalog entries and their blueprints, see the [Catalog object model](../../concepts/query-and-transform/catalog-object-model.md).
For the available configuration options, see the [Python table blueprint API reference](https://ref.rerun.io/docs/python/stable/blueprint_table/?speculative-link).
