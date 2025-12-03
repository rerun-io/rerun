---
title: Migrating from 0.27 to 0.28
order: 982
---

<!--   ^^^ this number must be _decremented_ when you copy/paste this file -->

## `Transform3D` no longer supports `axis_length` for visualizing coordinate axes

The `axis_length` parameter/method has been moved from `Transform3D` to a new `TransformAxes3D` archetype, which you can log alongside of `Transform3D`.
This new archetype also works with the `CoordinateFrame` archetype.

Existing `.rrd` recordings will be automatically migrated when opened (the migration converts `Transform3D:axis_length` components to `TransformAxes3D:axis_length`).

## `CoordinateFrame::frame_id` has been renamed to `CoordinateFrame::frame`

The `frame_id` component of `CoordinateFrame` has been renamed to just `frame`, because the component type `TransformFrameId` already conveys the information that this is an id.

Existing `.rrd` recordings will be automatically migrated when opened (the migration renames the `frame_id` component).

## Changes to `Transform3D`/`InstancePose3D` and `Pinhole`'s transform properties are now treated transactionally by the Viewer

If you previously updated only certain components of `Transform3D`/`InstancePose3D` and relied on previously logged
values remaining present,
you must now re-log those previous values every time you update the `Transform3D`/`InstancePose3D`.

If you always logged the same transform components on every log/send call or used the standard constructor of
`Transform3D`, no changes are required!

snippet: migration/transactional_transforms

`Pinhole`'s transform properties, `resolution` & `image_from_plane` as well its new `parent_frame` & `child_frame`,
fields are also affected by this change.
Again, this means that any change to any of `Pinhole`'s `resolution`/`image_from_plane`/`parent_frame`/`child_frame`,
will reset all of these fields.

### Details & motivation

We changed the way `Transform3D`, `InstancePose3D` & `Pinhole` are queried under the hood!

Usually, when querying any collection of components with latest-at semantics, we look for the latest update of each
individual component.
This is useful, for example, when you log a mesh and only change its texture over time:
a latest-at query at any point in time gets all the same vertex information, but the texture that is active at any given
point in time may change.

However, for `Transform3D`, this behavior can be very surprising,
as the typical expectation is that logging a `Transform3D` with only a rotation will not inherit previously logged
translations to the same path.
Previously, to work around this, all SDKs implemented the constructor of `Transform3D` such that it set all components
to empty arrays, thereby clearing everything that was logged before.
This caused significant memory (and networking) bloat, as well as needlessly convoluted displays in the viewer.
With the arrival of explicit ROS-style transform frames, per-component latest-at semantics can cause even more
surprising side effects.

Therefore, we decided to change the semantics of `Transform3D` such that any change to any of its components fully
resets the transform state.

For example, if you change its rotation and scale fields but do not write to translation, we will not look further back
in time to find the previous value of translation.
Instead, we assume that translation is not set at all (i.e., zero), deriving the new overall transform state only from
rotation and scale.
Naturally, if any update to a transform always changes the same components, this does not cause any changes other than
the simplification of not having to clear out all other components that may ever be set, thus reducing memory bloat both
on send and query!

## Python SDK: "partition" renamed to "segment" in catalog APIs

<!-- TODO(ab): as I roll more API updates, I'll keep that section up-to-date -->

In the `rerun.catalog` module, all APIs using "partition" terminology have been renamed to use "segment" instead.
The old APIs are deprecated and will be removed in a future release.

| Old API | New API |
|---------|---------|
| `DatasetEntry.partition_ids()` | `DatasetEntry.segment_ids()` |
| `DatasetEntry.partition_table()` | `DatasetEntry.segment_table()` |
| `DatasetEntry.partition_url()` | `DatasetEntry.segment_url()` |
| `DatasetEntry.download_partition()` | `DatasetEntry.download_segment()` |
| `DatasetEntry.default_blueprint_partition_id()` | `DatasetEntry.default_blueprint_segment_id()` |
| `DatasetEntry.set_default_blueprint_partition_id()` | `DatasetEntry.set_default_blueprint_segment_id()` |
| `DataframeQueryView.filter_partition_id()` | `DataframeQueryView.filter_segment_id()` |

The DataFusion utility functions in `rerun.utilities.datafusion.functions.url_generation` have also been renamed:

| Old API | New API |
|---------|---------|
| `partition_url()` | `segment_url()` |
| `partition_url_udf()` | `segment_url_udf()` |
| `partition_url_with_timeref_udf()` | `segment_url_with_timeref_udf()` |

The partition table columns have also been renamed from `rerun_partition_id` to `rerun_segment_id`.

Additionally, the `partition_id` field on viewer event classes has been renamed to `segment_id`:

```python
# Before (0.27)
def on_event(event):
    print(event.partition_id)

# After (0.28)
def on_event(event):
    print(event.segment_id)
```

This affects `PlayEvent`, `PauseEvent`, `TimeUpdateEvent`, `TimelineChangeEvent`, `SelectionChangeEvent`, and `RecordingOpenEvent`.
