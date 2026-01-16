---
title: Catalog object model
order: 50
---

This page covers the Data Platform's object model. For logging and recording basics, see [Recordings](../logging-and-ingestion/recordings.md). For API details, see the [Catalog SDK reference](https://ref.rerun.io/docs/python/stable/common/catalog/).


## Catalog

We refer to the contents store in a given instance of the Data Platform as the _catalog_.
The catalog contains top-level objects called _entries_.

There are currently two types of entries: **tables** and **datasets**.
Each are described in more details below.

Entries share a few common properties:
- **id**: a globally unique identifier
- **name**: a user-provided name, which must be unique within the catalog

The id is immutable, but the name can be changed provided it remains unique.


## Table entries

Table entries model a single table of data.
They use the Arrow data model (TODO link), so it is logically equivalent to an Arrow table (TODO link), or, equivalently, a ordered collection of Arrow record batches (TODO link, TODO check that this equivalency is actually correct).
As a result, tables posses an Arrow schema (TODO link).

Tables support the following mutation operations through the Catalog SDK:
- _append_: add new rows to the table
- _overwrite_: replace the entire table with new data
- _upsert_: replace existing rows (based on an index column) with new data

Thanks to DataFusion (TOOD Link), tables also support most database operations such as querying, filtering, joining, etc.

## Datasets

Dataset entries model a collection of Rerun data organized in "episodes," such as recorded runs of a given robotic task.
These episodes within datasets are called _segments_, which are identified by a segment ID.

Segments are added to datasets by the process of _registering_ a `.rrd` (typically stored in some object store such as S3) to the dataset using the Catalog SDK.
The recording ID of the `.rrd` file is used as segment ID.

`.rrd`s registered to a given segment are organised by layers, identified with layer name.
By default, the `"base"` layer name is used.
Rregistering two `.rrd` with the same recording ID (that is, with same segment ID) to the same dataset, and using the same layer name, will result in the second `.rrd` overwriting the first.
Additive registration can be achieved by using different layer names for different `.rrd`s with the same recording ID/segment ID.

TODO: try to make a d2 diagram to make sense of all that. two big boxes: catalog / object store. nested dataset/seg/layer in the former, rrd in the latter.

Layers are immutable and can only be overwritten by registering a new `.rrd` file. In other words, datasets support the following mutation operations:
- _create segment_: by registering a `.rrd` with "new" recording ID
- _apped to segment_: by registering a `.rrd` with a matching recording ID to a new layer name
- _overwrite segment layer_: by registering a `.rrd` with a matching recording ID to an existing layer name


### Dataset as an imaginary table

Datasets are based on the Rerun data model (TODO see eg link to chunk page), which aims to capture the unorganized nature typical of physical data. Data can be out-of-order, is multimodal in nature, organized along multiple independent timelines, and logged at different, possibly varying frequencies. Storing such data in a single table would be very impractical (which is why the Rerun data model exists in the first place). However, one could is very inconvenient, it is conceptually possible.  


### Schema

Datasets are based on the Rerun data model, which essentially consists of a collection of chunks of Arrow data (TODO link page).
These chunks hold data for various entities and components (TODO link) corresponding to various indexes (or timelines) (TODO link to something if you can).
A given collection of chunks, say, a dataset segment, defines an Arrow schema.
We refer to this as _schema-on-read_, because the schema proceeds from the data, and not the other way around.
This differs from the table model, where the schema is defined upfront (_schema-on-write_).

In this context, the schema of a dataset is the union of schemas of its segments, which themselves are the union of the schemas of their layers.

Datasets maintain a minimal level of schema self-consistency.
Registering a `.rrd` whose schema is incompatible with the current dataset schema will result in an error.
In this context, _incompatible_ means that the schema of the new `.rrd` contains a column for the same entity, archetype, and component, but with a different Arrow type.
Such occurrence is rare, and practically impossible in practice when using standard Rerun archetypes.


### Blueprints

Dataset can be assigned a blueprint.
In that case, the blueprint is applied to all segments of the dataset when visualized in the Rerun Viewer.


### Use cases for multiple layers

- Extending a segment with derived or computed data
- Adding annotations or labels
- Attaching metadata from different sources

