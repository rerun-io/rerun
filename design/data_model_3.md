# Data Model 3
The core of the third iteration of the data model and data store.

## Problems with previous model

Model 2 introduced the "batch" concept for efficiently logging and indexing large amounts of data. It was designed as an optimization for point clouds. Instead of logging each point individually, you can log many in a "batch", all with the same time stamp. This allowed for efficient indexing, but only if we had the caveat that "only the latest batch counts" - i.e. each new batch (to the same entity path) would overwrite previous batches.

It turned out this "batches overwrites previous values" was a hidden strength. For instance, if you do a bunch of detections in a camera frame, you can log all of their bboxes in a batch and next frame you overwrite all the detections from the previous frame by logging a new batch.

The "batch" used the last index of the data-path to distinguish between each instance. You would therefore log to a path with a placeholder index in the last index, e.g. `camera/"left"/points/*/.pos` where `*` is a placeholder meaning "here comes the whole batch". The hope was that this would make for simpler naming of data. That is, `camera/"left"/points/42/.pos` would work as an index wether or not the user chose to log `points` as a batch or individually. However, this special-treating of the last index position lead to rather a complicated implementation. It also is a bad abstraction: the paths _look_ the same, but they do not _act_ the same.

To complicate things even further, we allowed some field of an object to be batched and others to be non-batched (e.g. log a batch os point cloud positions and then log their colors individually).

## Changes in the new model

In the new model, batches are first-class-citizens. Each object can either be "mono" or "multi" (batch). Mono-objects are referred to with just an entity path, but multi-objects have an added `InstanceIndex` to distinguish which instance of the multi-object it is.

All fields in a multi-object must be batch-logged. There is no mixing allowed.


## Details

The basic structure is this:

```
objpath[instance_index].field
```

where `[instance_index]` is omitted for mono-objects.

To log a mono-object to the `ObjPath` `point`, we log `point.pos`, `point.color`, etc (no `InstanceIndex`).

If we log a point cloud, we have an index for each point, and we log each field in a batch, i.e. log all `points[*].pos` at once, and all `points[*].color` at once.

**Instances**: A mono-object has only one _instance_, while multi-objects have many _instances_. So an instance is uniquely identified with an `ObjPath` plus an optional `InstanceIndex`. You can get a hierarchical view of each object where you can toggle their visibility on/off, but you can NOT do it per-instance.

Each `ObjPath` has an associated `ObjectType` which dictates how the `field`s are interpreted (and which fields are allowed). The `ObjectType` needs to be logged once before any data is logged to the stream.

Note how the details of `ObjPath` doesn't matter at all here. We don't care if there are indices in it or not. This makes the data store code a lot simpler.
