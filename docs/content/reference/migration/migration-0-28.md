---
title: Migrating from 0.27 to 0.28
order: 982
---

<!--   ^^^ this number must be _decremented_ when you copy/paste this file -->

## `Pose*` component types have been removed

The following component types have been removed in favor of their more general counterparts:

* `components.PoseTranslation3D` → `components.Translation3D`
* `components.PoseRotationQuat` → `components.RotationQuat`
* `components.PoseTransformMat3x3` → `components.TransformMat3x3`
* `components.PoseRotationAxisAngle` → `components.RotationAxisAngle`
* `components.PoseScale3D` →  `components.Scale3D`

Existing `.rrd` files will be automatically migrated when opened.

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

| Old API                                             | New API                                           |
|-----------------------------------------------------|---------------------------------------------------|
| `DatasetEntry.partition_ids()`                      | `DatasetEntry.segment_ids()`                      |
| `DatasetEntry.partition_table()`                    | `DatasetEntry.segment_table()`                    |
| `DatasetEntry.partition_url()`                      | `DatasetEntry.segment_url()`                      |
| `DatasetEntry.download_partition()`                 | `DatasetEntry.download_segment()`                 |
| `DatasetEntry.default_blueprint_partition_id()`     | `DatasetEntry.default_blueprint_segment_id()`     |
| `DatasetEntry.set_default_blueprint_partition_id()` | `DatasetEntry.set_default_blueprint_segment_id()` |
| `DataframeQueryView.filter_partition_id()`          | `DataframeQueryView.filter_segment_id()`          |

The DataFusion utility functions in `rerun.utilities.datafusion.functions.url_generation` have also been renamed:

| Old API                            | New API                          |
|------------------------------------|----------------------------------|
| `partition_url()`                  | `segment_url()`                  |
| `partition_url_udf()`              | `segment_url_udf()`              |
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

## Python SDK: catalog entry listing APIs renamed

The `CatalogClient` methods for listing catalog entries have been renamed for clarity:

| Old API                           | New API                    |
|-----------------------------------|----------------------------|
| `CatalogClient.all_entries()`     | `CatalogClient.entries()`  |
| `CatalogClient.dataset_entries()` | `CatalogClient.datasets()` |
| `CatalogClient.table_entries()`   | `CatalogClient.tables()`   |

The old methods are deprecated and will be removed in a future release.

Additionally, the new methods accept an optional `include_hidden` parameter:
- `datasets(include_hidden=True)`: includes blueprint datasets
- `tables(include_hidden=True)`: includes system tables (e.g., `__entries`)
- `entries(include_hidden=True)`: includes both

## Python SDK: removed DataFrame-returning entry listing methods

The following methods that returned `datafusion.DataFrame` objects have been removed without deprecation:

| Removed method                                   | Replacement                                                             |
|--------------------------------------------------|-------------------------------------------------------------------------|
| `CatalogClient.entries()` (returning DataFrame)  | `CatalogClient.get_table(name="__entries").df()`                        |
| `CatalogClient.datasets()` (returning DataFrame) | `CatalogClient.get_table(name="__entries").df()` filtered by entry kind |
| `CatalogClient.tables()` (returning DataFrame)   | `CatalogClient.get_table(name="__entries").df()` filtered by entry kind |

The new `entries()`, `datasets()`, and `tables()` methods now return lists of entry objects (`DatasetEntry` and `TableEntry`) instead of DataFrames. If you need DataFrame access to the raw entries table, use `client.get_table(name="__entries").df()`.

## Python SDK: entry name listing methods now support `include_hidden`

The `CatalogClient` methods for listing entry names now accept an optional `include_hidden` parameter, matching the behavior of `entries()`, `datasets()`, and `tables()`:

- `entry_names(include_hidden=True)`: includes hidden entries (blueprint datasets and system tables like `__entries`)
- `dataset_names(include_hidden=True)`: includes blueprint datasets
- `table_names(include_hidden=True)`: includes system tables (e.g., `__entries`)

## Python SDK: entry access methods renamed

The `CatalogClient` methods for accessing individual entries have been renamed:

| Old API                              | New API                        |
|--------------------------------------|--------------------------------|
| `CatalogClient.get_dataset_entry()`  | `CatalogClient.get_dataset()`  |
| `CatalogClient.get_table_entry()`    | `CatalogClient.get_table()`    |
| `CatalogClient.create_table_entry()` | `CatalogClient.create_table()` |

The existing `CatalogClient.create_dataset()` method is already aligned with the new naming scheme and remains unchanged.
The old methods are deprecated and will be removed in a future release.

## Python SDK: `get_table()` now returns `TableEntry` instead of DataFrame

The `CatalogClient.get_table()` method has been changed to return a `TableEntry` object instead of a `datafusion.DataFrame`. This is a **breaking change**.

```python
# Before (0.27)
df = client.get_table(name="my_table")  # returns DataFrame

# After (0.28)
table_entry = client.get_table(name="my_table")  # returns TableEntry
df = table_entry.df()  # call df() to get the DataFrame
```

This change aligns `get_table()` with `get_dataset()`, which returns a `DatasetEntry`. Both methods now consistently return entry objects that provide access to metadata and data.

## Python SDK: table write operations moved to `TableEntry`

Write operations for tables have been moved from `CatalogClient` to `TableEntry`. The new methods provide a cleaner API that operates directly on table entries:

| Old API                                              | New API                           |
|------------------------------------------------------|-----------------------------------|
| `CatalogClient.write_table(name, batches, mode)`     | `TableEntry.append(batches)`      |
|                                                      | `TableEntry.overwrite(batches)`   |
|                                                      | `TableEntry.upsert(batches)`      |
| `CatalogClient.append_to_table(name, batches)`       | `TableEntry.append(batches)`      |
| `CatalogClient.update_table(name, batches)`          | `TableEntry.upsert(batches)`      |

The old methods are deprecated and will be removed in a future release.

```python
# Before (0.27)
client.write_table("my_table", batches, TableInsertMode.APPEND)
client.append_to_table("my_table", batches)
client.update_table("my_table", batches)

# After (0.28)
table = client.get_table(name="my_table")
table.append(batches)
table.overwrite(batches)
table.upsert(batches)
```

The new `TableEntry` methods also support writing Python objects directly via keyword arguments:

```python
table.append(col1=[1, 2, 3], col2=["a", "b", "c"])
```

Note: `TableInsertMode` is no longer needed with the new API and will be removed in a future release.

## Python SDK: schema and column types moved to `rerun.catalog`

The `Schema` class and related column descriptor/selector types have moved from `rerun.dataframe` to `rerun.catalog`.

| Old import (0.27)                                       | New import (0.28)                                      |
|---------------------------------------------------------|--------------------------------------------------------|
| `from rerun.dataframe import Schema`                    | `from rerun.catalog import Schema`                     |
| `from rerun.dataframe import ComponentColumnDescriptor` | `from rerun.catalog import ComponentColumnDescriptor`  |
| `from rerun.dataframe import ComponentColumnSelector`   | `from rerun.catalog import ComponentColumnSelector`    |
| `from rerun.dataframe import IndexColumnDescriptor`     | `from rerun.catalog import IndexColumnDescriptor`      |
| `from rerun.dataframe import IndexColumnSelector`       | `from rerun.catalog import IndexColumnSelector`        |

The previous import paths are still supported but will be removed in a future release.
