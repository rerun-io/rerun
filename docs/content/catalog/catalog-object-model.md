---
title: Catalog object model
order: 100
description: How the catalog organizes datasets, segments, and entries
---

This page covers the catalog server's object model, which the open-source server and [Rerun Hub](hub.md) share; the page notes where they differ.
For logging and recording basics, see [Recordings](../log/data-model/recordings.md).
For API details, see the [Catalog SDK reference](https://ref.rerun.io/docs/python/stable/catalog/).


## Catalog

We refer to the contents stored in a given catalog server as the _catalog_.
The catalog contains top-level objects called _entries_.

There are currently two types of entries: **tables** and **datasets**.

Entries share a few common properties:
- **id**: a globally unique identifier
- **name**: a user-provided name, which must be unique within the catalog

### Renaming a catalog entry

The id of a catalog entry is immutable, but the name can be changed provided it remains unique.
In Python, call `set_name()` on an entry to rename it on the catalog server, for example:

```python
client = rr.catalog.CatalogClient(…)
dataset = client.get_dataset("old_name")
dataset.set_name("new_name")
```

### Structuring datasets

When working with larger amounts of data, it can be useful to organize catalog entries in a directory-like structure.
This can be done by using `.` delimiters in the names.
The screenshot below is an example of a dot-delimited dataset name showing up as a directory tree in the viewer's data source browser:

<picture>
  <img src="https://static.rerun.io/catalog_dirs/1a7f0c9a24f418857c94b46fa7fa39381155a91b/full.png" alt="Data source browser with the tree hiw_500, episodes, revision_1 and revision_2 beside a table of the two dataset entries">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/catalog_dirs/1a7f0c9a24f418857c94b46fa7fa39381155a91b/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/catalog_dirs/1a7f0c9a24f418857c94b46fa7fa39381155a91b/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/catalog_dirs/1a7f0c9a24f418857c94b46fa7fa39381155a91b/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/catalog_dirs/1a7f0c9a24f418857c94b46fa7fa39381155a91b/1200w.png">
</picture>

## Table entries

Table entries model a single table of data.
They use the [Arrow data model](https://arrow.apache.org/docs/format/Columnar.html), so a table is logically equivalent to an [Arrow table](https://arrow.apache.org/docs/python/generated/pyarrow.Table.html).
As a result, tables possess an [Arrow schema](https://arrow.apache.org/docs/python/generated/pyarrow.Schema.html).

Through the Catalog SDK, you can append rows to a table, overwrite it, or upsert rows based on an index column.
Thanks to [DataFusion](https://datafusion.apache.org/), tables also support most database operations such as querying, filtering, joining, etc.
See [Catalog tables](catalog-tables.md) for how to create, read, and update them, and [Storing query results in a catalog table](store-query-results.md) for a complete example.

## Datasets

Dataset entries model a collection of Rerun data organized in episodes such as recorded runs of a given robotic task.
These episodes within datasets are called _segments_, which are identified by a segment ID.

Segments are added to datasets by the process of _registering_ a [recording](../log/data-model/recordings.md) (typically stored in some object store such as S3) to the dataset using the Catalog SDK.
By default, the recording ID of the `.rrd` file is used as its segment ID.
To register it under another segment ID, pass `segment_id` to `register()`: see [How can I register a recording under another segment ID?](layers.md#how-can-i-register-a-recording-under-another-segment-id).
Keep one recording per registered `.rrd` file.
Rerun Hub registers each file as a single segment, named after the first recording in it.

Recordings registered to a given segment are organized by layers, identified by a layer name.
By default, the `"base"` layer name is used.
Registering two `.rrd` files under the same segment ID to the same dataset, and using the same layer name, throws an error by default.
Pass `on_duplicate=OnDuplicateSegmentLayer.REPLACE` to `register()` to overwrite the existing layer, or `SKIP` to leave it in place.
Additive registration can be achieved by using different layer names for different `.rrd`s registered under the same segment ID.

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/87994b13acba8b61778eca16b947454f6d4380d9_diagram-catalog-object-model-dark.svg" alt="Diagram of a catalog dataset with segment_a and segment_b, whose base and extra layers each point to an .rrd file in an object store">
  <img class="d2-light" src="https://static.rerun.io/139785ff1666d48a7caf4ad48a7b4226a6f37011_diagram-catalog-object-model-light.svg" alt="Diagram of a catalog dataset with segment_a and segment_b, whose base and extra layers each point to an .rrd file in an object store">
</div>

Layers are immutable and can only be overwritten by registering a new `.rrd` file. In other words, datasets support the following mutation operations:
- _create segment_: by registering a `.rrd` under a "new" segment ID
- _append to segment_: by registering a `.rrd` under a matching segment ID to a new layer name
- _overwrite segment layer_: by registering a `.rrd` under a matching segment ID to an existing layer name, with `on_duplicate=OnDuplicateSegmentLayer.REPLACE`

### Managing segment layers with the Python catalog API

Use `DatasetEntry.segment()` to get a handle to a segment, inspect its current layers, or get its Viewer URL.
Call `delete_layer()` to unregister one layer from that segment, then wait for the returned handle to track completion.
Unregistering removes catalog references but does not delete the underlying source objects.

snippet: concepts/query-and-transform/catalog_object_model[segment_layers]

To inspect a named layer across the dataset, use `dataset.layer(name).segment_ids()`.
The layer handle reads current membership on each call, including registrations and removals made after the handle was created.
See [finding segments in a layer](layers.md#how-can-i-find-the-segments-containing-a-layer) for an example.

See the [`Segment` API reference](https://ref.rerun.io/docs/python/stable/catalog/#rerun.catalog.Segment) for details.

### Schema

Datasets are based on the Rerun data model, which consists of a collection of [chunks](../log/chunks-and-rrd/chunks.md) of Arrow data.
These chunks hold data for various [entities and components](../log/data-model/entity-component.md) corresponding to various indexes (or [timelines](../log/data-model/timelines.md)).
A given collection of chunks, say, a dataset segment, defines an Arrow schema.
We refer to this as _schema-on-read_, because the schema proceeds from the data, and not the other way around.
This differs from the table model, where the schema is defined upfront (_schema-on-write_).

In this context, the schema of a dataset is the union of schemas of its segments, which themselves are the union of the schemas of their layers.

Use `segment.schema()` to fetch a snapshot of one segment's schema across its registered layers.
Referenced assets are excluded from the segment's schema.
Use `dataset.layer(name).schema()` to fetch the union of one layer's schemas across the segments that contain it.

snippet: concepts/query-and-transform/catalog_object_model[schema]

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/a2425d9909b1766edff1b2940d66331f8f54d512_diagram-schema-stack-dark.svg" alt="Diagram of my_dataset schema stacked on segment_a schema (base and extra layer schemas) and segment_b schema (base schema)">
  <img class="d2-light" src="https://static.rerun.io/df137081af950d481ad5b27ae900910d286a2ca3_diagram-schema-stack-light.svg" alt="Diagram of my_dataset schema stacked on segment_a schema (base and extra layer schemas) and segment_b schema (base schema)">
</div>

Datasets maintain a minimal level of schema self-consistency.
Registering a `.rrd` whose schema is incompatible with the current dataset schema will result in an error.
In this context, _incompatible_ means that the schema of the new `.rrd` contains a column for the same entity, archetype, and component, but with a different Arrow type.
Such an occurrence is rare, and practically impossible when using standard Rerun archetypes.


### Blueprints

You can add blueprints to a dataset by registering `.rbl` blueprint files, typically stored in object storage.
A dedicated API exists for this in the Catalog SDK: [`DatasetEntry.register_blueprint()`](https://ref.rerun.io/docs/python/stable/catalog/#rerun.catalog.DatasetEntry.register_blueprint).
You can also set one as the default, which is then loaded with each segment of the dataset.

The registered blueprints show up in the Viewer in a [dropdown in the top bar](../viewer/inspect-recordings/recording-layout.md#select-a-recording-blueprint-for-a-dataset).

A separate **table blueprint** can be used to configure the segment table itself: columns, card layouts, and embedded segment previews.
See [Configure table layouts and segment previews](../viewer/browse-datasets/table-layout.md) for the complete save-and-register workflow.


### Assets

An asset is static data registered on a dataset and shared by its segments, such as a robot URDF, a room mesh, or a calibration.
Registering it once as an asset means it does not have to be copied into every recording.

Like blueprints, assets live in their own dataset, owned by the dataset they belong to.
Deleting the dataset also deletes its asset dataset and the asset storage.

Assets are registered from a `.rrd` file that the server can read, using [`DatasetEntry.register_asset()`](https://ref.rerun.io/docs/python/stable/catalog/#rerun.catalog.DatasetEntry.register_asset):

snippet: concepts/query-and-transform/catalog_object_model[assets]

Asset datasets have some restrictions to keep them light, and the server rejects registrations that:
- contain temporal data, since an asset may only contain static data
- are larger than 300 MiB
- would bring the dataset above 12 assets

Assets can apply to all segments except selected ones (`mode="opt_out"`, the default), or only to selected segments (`mode="opt_in"`).
Choose coverage during registration or change it later using `add_segments_to_asset()` and `remove_segments_from_asset()`.

When the Rerun Viewer opens a segment, it loads the assets that apply to it along with the segment's own data.
The asset data is cached, so opening another segment of the same dataset does not download it again.
The assets tab of a dataset lists the registered assets and their coverage, and lets you register and unregister them and edit their coverage in the Viewer.
If the server accepts uploads, the viewer can also upload a `.rrd` from your machine and register it as an asset.

Assets also show up in the [python chunk processing api](../ingest/chunk-processing-api.md) if `include_assets` is `True`:

snippet: concepts/query-and-transform/catalog_object_model[segment_store]

Fetching an asset's manifest costs one request per asset, so `include_assets=False` is worth it when you only care about the segment's own data.

The [dataframe query APIs](../query-and-transform/dataframes/dataframe-queries.md) are the exception: they stay on the dataset's own segments, so assets never show up there.
[`DatasetEntry.schema()`](https://ref.rerun.io/docs/python/stable/catalog/#rerun.catalog.DatasetEntry.schema), [`DatasetEntry.segment_ids()`](https://ref.rerun.io/docs/python/stable/catalog/#rerun.catalog.DatasetEntry.segment_ids), [`DatasetEntry.segment_table()`](https://ref.rerun.io/docs/python/stable/catalog/#rerun.catalog.DatasetEntry.segment_table) and [`DatasetEntry.reader()`](https://ref.rerun.io/docs/python/stable/catalog/#rerun.catalog.DatasetEntry.reader) all ignore the asset dataset.
The asset dataset is hidden, so it is also left out of [`CatalogClient.datasets()`](https://ref.rerun.io/docs/python/stable/catalog/#rerun.catalog.CatalogClient.datasets) unless you pass `include_hidden=True`.

To query asset data, target the asset dataset itself, which is a regular dataset with one segment per registered asset:

snippet: concepts/query-and-transform/catalog_object_model[asset_dataset]
