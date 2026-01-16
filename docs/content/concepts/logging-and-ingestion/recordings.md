---
title: Recordings
order: 100
---

Rerun organizes data at two levels: **recordings** from the Logging SDK, and **datasets** managed by the Data Platform. These concepts connect when recordings are registered to datasets.

## Recordings

In general, a recording is a semantic collection of related data, with some associated metadata.
From the logging perspective, in its simplest form, a recording can be thought of as a single `.rrd` file (although Rerun data can also be [directly streamed](../how-does-rerun-work.md#stream-to-viewer)). At logging time, recordings are identified by a _recording ID_ and _application ID_. What these mean in different contexts is explained below.

### Logical vs physical recordings

The recording/file analogy comes short of describing how the Rerun Viewer handles data.

When the Viewer receives data, whether by loading a `.rrd` file or an incoming logging stream, it pools the corresponding data by recording ID and application ID.
This can be thought of as a logical recording, even though its source might be, e.g., multiple `.rrd` files.
This implicit merging semantics also implies that, from the perspective of the Viewer, recordings are never "completed."
They can always be appended to with new data.
This enables the [distributed logging workflows](#distributed-recordings) described below.

In its UI, the Viewer presents (logical) recordings sharing the same application ID as related.
In particular, they share the same [blueprint](../visualization/blueprints.md).


### Recordings on the Data Platform

The Data Platform has a slightly different object model, which you can read more about in [Catalog object model](../query-and-transform/catalog-object-model.md).

In a nutshell, datasets are top-level objects that group semantically related episodes of data, which we call _segments_.
For example, it can be multiple recordings of the same robotic task.
Blueprints can optionally be assigned to datasets, so all segments in a dataset share the same blueprint.

Populating a dataset happens by registering recordings to it using the Catalog SDK.
Its recording ID becomes the segment ID, and its application ID is disregarded.

Segments can contain multiple _layers_ identified by their name, each backed by a `.rrd` file.
This again allows pooling multiple physical recordings into a single (logical) segment.


### Distributed recordings

Both the Viewer's implicit merging semantics and the Data Platform's layer system enable distributed logging workflows. Multiple processes or machines can produce separate `.rrd` files that share the same recording ID and application ID.

When these files are loaded into the Viewer, they are treated as a single logical recording. Alternatively, when using the Data Platform, these files can be registered to separate layers. This enables workflows where data collection is distributed across multiple sources but visualized as a unified set of data.

You can learn more about this in the [shared recordings guide](../../howto/logging-and-ingestion/shared-recordings.md).


### Storage formats

Rerun recordings are typically stored in `.rrd` files. [Blueprints](../visualization/blueprints.md) are also recordings, albeit ones containing layout information instead of data. By convention, the `.rbl` file extension is used for blueprints.


## Application IDs

Rerun recordings also have an _application ID_ in their metadata.
Application IDs are arbitrary user-defined strings set when initializing the SDK:

snippet: tutorials/custom-application-id

### When Application IDs matter

Application IDs are used by the Viewer when loading recordings directly (not via the Data Platform):

- The Viewer stores blueprints per application ID
- Different recordings share the same blueprint if they share the same application ID
- Recordings are grouped by application ID in the Viewer UI

As stated above, application IDs are disregarded when registering recordings to the Data Platform. See [Recordings on the Data Platform](#recordings-on-the-data-platform) above.

Check out the API to learn more about SDK initialization:
- [🐍 Python](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.init)
- [🦀 Rust](https://docs.rs/rerun/latest/rerun/struct.RecordingStreamBuilder.html#method.new)
- [🌊 C++](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#abda6202900fa439fe5c27f7aa0d1105a)


## Recording IDs

By default, a random recording ID is generated each time you start logging.
This means that, by default, separate logging sessions will produce separate (logical) recording when loaded in the Viewer, and separate segments when registered to a dataset.

You can override the default recording ID when initializing the SDK (or the recording stream):

snippet: tutorials/custom-recording-id

This enables the distributed logging workflow described above, as well as assigning specific segment ID for recordings to be registered to datasets.

<!-- TODO(RR-2168): update this section when segment id override is available -->
