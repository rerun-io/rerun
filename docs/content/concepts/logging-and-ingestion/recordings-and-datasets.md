---
title: Recordings and datasets
order: 100
---

TODO: I, again, changed my mind. name this file "Recordings" / recordings.md

Rerun organizes data at two levels: **recordings** from the Logging SDK, and **datasets** managed by the Data Platform. These concepts connect when recordings are registered to datasets.

## Recordings

In general, a recording is a semantic collection of related data, with some associated metadata.
From the logging perspective, in its simplest form, a recording can be thought of as a single `.rrd` file (although Rerun data can also be directly streamed TODO link to how-does-rerun-work#stream-to-viewer). At logging time, recordings are identified by a _Recording ID_ and _Application ID_. What these mean in different contexts is explained below.

### Logical vs physical recordings

The recording-file analogy comes short of describing how the Rerun Viewer handles data.

When the viewer receives data, whether by loading a `.rrd` file or an incoming logging stream, it pools the corresponding data by recording ID and application ID.
This can be thought of a logical recording, even though its source might be, for example, multiple `.rrd` files.
This implicit merging semantics also implies that, from the perspective of the viewer, recordings are never "completed."
They can always be appended to with new data.
This enables distributed logging workflows (see TODO link paragraph below)

In its UI, the viewer presents (logical) recordings sharing the same application ID as related.
In particular, they share the same blueprint (TODO link).


### Recordings on the data platform

TODO move here the succint description of how recording are in the dataplatofrm, with link to document 2


### Distributed recordings

Both the viewer's implicit merging semantics and the data platform's layer system enable distributed logging workflows. Multiple processes or machines can produce separate `.rrd` files that share the same Recording ID and Application ID.

When these files are loaded into the Viewer, they are treated as a single logical recording. Alternatively, when using the data platform, these files can be registered to separate layers. This enables workflows where data collection is distributed across multiple sources but visualized as a unified set of data.

You can learn more about this in the [shared recordings guide](../../howto/logging-and-ingestion/shared-recordings.md).


### Storage formats

Rerun recordings are typically stored in `.rrd` files. [Blueprints](../visualization/blueprints.md) are also recordings, albeit ones containing layout information instead of data. By convention, the `.rbl` file extension is used for blueprints.


## Application IDs

Rerun recordings also have an _Application ID_ in their metadata.
Application IDs are arbitrary user-defined strings set when initializing the SDK:

snippet: tutorials/custom-application-id

### When Application IDs matter

Application IDs are used by the Viewer when loading recordings directly (not via the Data Platform):

- The Viewer stores blueprints per Application ID
- Different recordings share the same blueprint if they share the same Application ID
- Recordings are grouped by Application ID in the Viewer UI

Application IDs are disregarded when registering recordings to the Data Platform. See [Datasets](#datasets) below.

Check out the API to learn more about SDK initialization:
- [🐍 Python](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.init)
- [🦀 Rust](https://docs.rs/rerun/latest/rerun/struct.RecordingStreamBuilder.html#method.new)
- [🌊 C++](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#abda6202900fa439fe5c27f7aa0d1105a)


## Recording IDs

By default, a random Recording ID is generated each time you start logging. You can optionally specify a recording ID when initializing the SDK—this is required for [distributed recordings](#distributed-recordings).

When a recording is registered to the Data Platform, its recording ID becomes the segment ID.

<!-- TOOD(RR-2168): update this section when segment id override is available -->


TODO: the following content needs to be moved up
## Datasets

Datasets are collections of recordings managed by the Data Platform. They provide persistent storage and indexing for large-scale data.

- Each dataset has a unique name within a catalog
- Blueprints attach to datasets—all recordings in a dataset share the same blueprint

### Segments and layers

When registered, a recording becomes a _segment_ in the dataset:
- The Recording ID is used as the Segment ID
- The Application ID is not used—the blueprint comes from the dataset instead
- Segments can contain multiple _layers_ (for derived data, updates)

For a detailed explanation of the Data Platform's object model, see [Catalog object model](../query-and-transform/catalog-object-model.md).
