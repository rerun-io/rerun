---
title: Catalog object model
order: 50
---

This page covers the Data Platform's object model. For logging and recording basics, see [Recordings and datasets](../logging-and-ingestion/recordings-and-datasets.md). For API details, see the [Catalog SDK reference](https://ref.rerun.io/docs/python/stable/common/catalog/).


## Catalog

A catalog is the top-level container in the Data Platform. It contains _entries_—the objects you work with.


## Entries

Entries are the objects stored in a catalog. Each entry has a unique name within its catalog.

There are two types of entries: **tables** and **datasets**.


## Tables

Tables are Arrow tables with persistent storage. They are mutable: you can append to them and modify their contents. Tables are queryable via DataFusion.

Use cases include storing metadata, configuration, and simple tabular data.


## Datasets

Datasets are collections of Rerun recordings. Blueprints attach at the dataset level, so all recordings in a dataset share the same blueprint. Datasets support filtering and search.

Datasets are composed of _segments_.


## Segments

A segment represents a registered recording in a dataset. It is identified by a Segment ID, which equals the Recording ID from the original recording. The Application ID from the recording is not used.

Each segment is composed of one or more _layers_.


## Layers

Layers are named components within a segment. Each layer maps to one or more `.rrd` files.

### Default behavior

Upon registration, a recording is added to the `base` layer. Single-layer segments are the common case.

### Use cases for multiple layers

- Extending a segment with derived or computed data
- Adding annotations or labels
- Attaching metadata from different sources

### Immutability model

Layers are the core unit of segment updates. A layer is immutable—it can only be overwritten, not modified. To update data in a segment, you overwrite the entire layer.
