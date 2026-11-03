---
title: "0.42"
order: 950
---

## Highlights

### Raw data view

Inspect an entity's components directly with the new [Raw Data view](../reference/types/views/raw_data_view.md), including nested fields and individual instances.
The view allows you to pick which components to show, switch between compact and spacious layouts, and step through changes to the displayed data.
Configure it in the Viewer or [with a Python blueprint](../reference/types/views/raw_data_view.md#example).

<picture>
  <img src="https://static.rerun.io/raw-data-view/a5701c844bfba884f8897b112f0e2589d1bb164a/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/raw-data-view/a5701c844bfba884f8897b112f0e2589d1bb164a/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/raw-data-view/a5701c844bfba884f8897b112f0e2589d1bb164a/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/raw-data-view/a5701c844bfba884f8897b112f0e2589d1bb164a/1024w.png">
</picture>

### Schema tab for datasets

The dataset page in the redap browser has a new "Schema" tab, showing the combined [schema](../concepts/query-and-transform/catalog-object-model.md#schema) of all [layers](../concepts/query-and-transform/catalog-object-model.md#managing-segment-layers-with-the-python-catalog-api) in the dataset.

Entities are listed as a searchable tree, and the components at the selected entity are shown next to it.
The archetype and data type of each component can be shown as extra columns.

<picture>
  <img src="https://static.rerun.io/dataset_schema_tab/0d89f852f6fc0d912e2b96e426b31c0a8ad66a9f/full.png" alt="The schema tab of a dataset, with the entity tree on the left and the components of the selected entity on the right">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/dataset_schema_tab/0d89f852f6fc0d912e2b96e426b31c0a8ad66a9f/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/dataset_schema_tab/0d89f852f6fc0d912e2b96e426b31c0a8ad66a9f/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/dataset_schema_tab/0d89f852f6fc0d912e2b96e426b31c0a8ad66a9f/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/dataset_schema_tab/0d89f852f6fc0d912e2b96e426b31c0a8ad66a9f/1200w.png">
</picture>

## Other

- **Hovering a tensor shows a zoomed preview and element values**: Hovering a tensor in the [tensor view](../reference/types/views/tensor_view.md) now shows a magnified view of the surrounding elements, along with the index and value of the element under the cursor, like hovering an image in the 2D view does.

  <img src="https://static.rerun.io/tensor-view-hover/9e0adf6a485257e0e0721130bd28f3c557e7d005/full.png" alt="Display tooltip hovering over tensor index and showing matching value.">
- **`Component.table_field()`**: `rr.components.Flag.table_field("reviewed", editable=True)` returns a catalog table schema field with a Rerun component type, which gives the column semantic meaning. The Viewer uses it to pick the UI for the column. With `editable=True`, the column can be edited without a table blueprint. See [Give columns semantic meaning](../howto/query-and-transform/catalog-tables.md#give-columns-semantic-meaning).

  ```python
  schema = pa.schema([
      pa.field("id", pa.int64(), metadata={rr.SORBET_IS_TABLE_INDEX: "true"}),
      rr.components.Url.table_field("link"),
      rr.components.Flag.table_field("reviewed", editable=True),
      rr.components.Text.table_field("notes", editable=True),
  ])
  ```
- **Layer names show as tags in tables**: The layer names column of a segment table now shows each layer as a tag.

  <img src="https://static.rerun.io/layer_names_column/349805a81a4487a768bf34577db4c062ccb8e24c/full.png" alt="Segment table with layer names shown as tags, and a tooltip listing all layer names of a segment">
- **Clickable catalog folder breadcrumbs**: Catalog folders can now be navigated with clickable breadcrumbs.
- **Copy link to catalog folder and table**: Right-click a catalog folder or table in the recording panel to copy a link to it. Folders can also copy their name.
- **Merge repeated notifications**: Repeats of the same notification merge into one, which shows how many times it happened and keeps the details of each occurrence.
- **`Layer.schema()`**: `dataset.layer(name).schema()` returns the schema of one layer, merged across the segments that contain it. It requires a server running Rerun 0.41 or Rerun Hub 0.20 or later.
- **`DatasetSelection.info()`**: `DatasetSelection.info()` summarizes a selection like `DatasetEntry.info()` does for a whole dataset. For a selection with a segment filter, this requires a server running Rerun 0.41 or Rerun Hub 0.20 or later.
- **CORS allows rerun.io subdomains by default**: The gRPC server and proxy now accept browser connections from all `rerun.io` subdomains by default, so the web viewer on `app.rerun.io` can connect to a local `serve_grpc` without `--cors-allow-origin`.

## Breaking changes

### `DatasetSelection.schema()` covers only the selected segments

The schema of a selection with a segment filter now holds only the columns of the selected segments.
Previously, it held the columns of every segment of the dataset.
A segment filter requires a server running Rerun 0.41 or Rerun Hub 0.20 or later, and older servers raise an error.

#### Migration

To get the schema of the whole dataset, as before, ask the dataset instead of the selection:

```python
selection = dataset.filter_segments(["first", "second"])

# Before: the columns of every segment. Now: the columns of `first` and `second`.
schema = selection.schema()

# The columns of every segment, before and now.
schema = selection.dataset.schema()
```

`selection.dataset.schema()` ignores content filters: apply them to the dataset, as in `dataset.filter_contents("/robot/**").schema()`.

---

Looking for an older release? See the [migration guides for 0.33 and earlier](../reference/migration.md).
