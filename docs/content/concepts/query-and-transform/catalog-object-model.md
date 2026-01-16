---
title: Catalog object model
order: 50
---

This page covers the Data Platform's object model. For logging and recording basics, see [Recordings](../logging-and-ingestion/recordings.md). For API details, see the [Catalog SDK reference](https://ref.rerun.io/docs/python/stable/common/catalog/).


## Catalog

We refer to the contents stored in a given instance of the Data Platform as the _catalog_.
The catalog contains top-level objects called _entries_.

There are currently two types of entries: **tables** and **datasets**.
Each is described in more detail below.

Entries share a few common properties:
- **id**: a globally unique identifier
- **name**: a user-provided name, which must be unique within the catalog

The id is immutable, but the name can be changed provided it remains unique.


## Table entries

Table entries model a single table of data.
They use the [Arrow data model](https://arrow.apache.org/docs/format/Columnar.html), so a table is logically equivalent to an [Arrow table](https://arrow.apache.org/docs/python/generated/pyarrow.Table.html), or, equivalently, an ordered collection of [Arrow record batches](https://arrow.apache.org/docs/python/generated/pyarrow.RecordBatch.html).
As a result, tables possess an [Arrow schema](https://arrow.apache.org/docs/python/generated/pyarrow.Schema.html).

Tables support the following mutation operations through the Catalog SDK:
- _append_: add new rows to the table
- _overwrite_: replace the entire table with new data
- _upsert_: replace existing rows (based on an index column) with new data

Thanks to [DataFusion](https://datafusion.apache.org/), tables also support most database operations such as querying, filtering, joining, etc.

## Datasets

Dataset entries model a collection of Rerun data organized in "episodes," such as recorded runs of a given robotic task.
These episodes within datasets are called _segments_, which are identified by a segment ID.

Segments are added to datasets by the process of _registering_ a `.rrd` (typically stored in some object store such as S3) to the dataset using the Catalog SDK.
The recording ID of the `.rrd` file is used as segment ID.

`.rrd`s registered to a given segment are organized by layers, identified by a layer name.
By default, the `"base"` layer name is used.
Registering two `.rrd` files with the same recording ID (that is, with the same segment ID) to the same dataset, and using the same layer name, will result in the second `.rrd` overwriting the first.
Additive registration can be achieved by using different layer names for different `.rrd`s with the same recording ID/segment ID.

```d2
direction: left

Catalog: {
  shape: cylinder

  my_dataset: {
    label: "my_dataset"

    segment_a: {
      label: "segment_a"

      base: {
        label: "layer\n\"base\""
        shape: parallelogram
      }
    }

    segment_b: {
      label: "segment_b"

      base: {
        label: "layer\n\"base\""
        shape: parallelogram
      }
      annotations: {
        label: "layer\n\"extra\""
        shape: parallelogram
      }
    }
  }
}

Object Store: {
  shape: cylinder
  
  "recording_a.rrd": {
    shape: page
  }
  "recording_b.rrd": {
    shape: page
  }
  "extra_b.rrd": {
    shape: page
  }
}

Object Store."recording_a.rrd" -> Catalog.my_dataset.segment_a.base
Object Store."recording_b.rrd" -> Catalog.my_dataset.segment_b.base
Object Store."extra_b.rrd" -> Catalog.my_dataset.segment_b.annotations
```

Layers are immutable and can only be overwritten by registering a new `.rrd` file. In other words, datasets support the following mutation operations:
- _create segment_: by registering a `.rrd` with a "new" recording ID
- _append to segment_: by registering a `.rrd` with a matching recording ID to a new layer name
- _overwrite segment layer_: by registering a `.rrd` with a matching recording ID to an existing layer name


### Schema

Datasets are based on the Rerun data model, which essentially consists of a collection of [chunks](../logging-and-ingestion/chunks.md) of Arrow data.
These chunks hold data for various [entities and components](../logging-and-ingestion/entity-component.md) corresponding to various indexes (or [timelines](../logging-and-ingestion/timelines.md)).
A given collection of chunks, say, a dataset segment, defines an Arrow schema.
We refer to this as _schema-on-read_, because the schema proceeds from the data, and not the other way around.
This differs from the table model, where the schema is defined upfront (_schema-on-write_).

In this context, the schema of a dataset is the union of schemas of its segments, which themselves are the union of the schemas of their layers.

Datasets maintain a minimal level of schema self-consistency.
Registering a `.rrd` whose schema is incompatible with the current dataset schema will result in an error.
In this context, _incompatible_ means that the schema of the new `.rrd` contains a column for the same entity, archetype, and component, but with a different Arrow type.
Such an occurrence is rare, and practically impossible when using standard Rerun archetypes.


### Blueprints

A dataset can be assigned a blueprint.
In that case, the blueprint is applied to all segments of the dataset when visualized in the Rerun Viewer.