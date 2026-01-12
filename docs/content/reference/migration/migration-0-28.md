---
title: Migrating from 0.27 to 0.28
order: 982
---

<!--   ^^^ this number must be _decremented_ when you copy/paste this file -->

## Deprecated components and APIs have been removed

This release includes a major cleanup where many components, methods, and APIs that were marked as deprecated before version 0.27 have been removed. If you have been using any deprecated APIs, you will need to update your code to use the recommended replacements.

### Removed deprecated components and methods

The following previously deprecated items have been removed:

#### Rust SDK
- `TimeColumn::new_seconds()` and `TimeColumn::new_nanos()` methods
- `Timeline::new_temporal()` method
- `Asset3D::from_file()` method (use `Asset3D::from_file_path()` instead)
- `AssetVideo::read_frame_timestamps_ns()` method (was renamed to `read_frame_timestamps_nanos()`)
- `Image::from_file_path()` and `Image::from_file_contents()` methods (use `EncodedImage` equivalents instead)
- Various deprecated methods on `Pinhole` archetype (use component-specific methods instead)
- `Scale3D::Uniform()` and `Scale3D::ThreeD()` methods (use `Scale3D::uniform()` and `Scale3D::from()` instead)
- `VideoTimestamp::from_seconds()`, `from_milliseconds()`, and `from_nanoseconds()` methods (use `from_secs()`, `from_millis()`, and `from_nanos()` instead)
- `Angle::Degrees()` and `Angle::Radians()` methods (use `Angle::from_degrees()` and `Angle::from_radians()` instead)
- `RecordingStream::serve_web()` method (use combination of `serve_grpc()` and `serve_web_viewer()`)
- `RecordingStream::set_time_secs()` and `set_time_nanos()` methods (use `set_time()` with appropriate time types)

#### Python SDK
- `ImageEncoded` and `ImageFormat` classes (use `EncodedImage` instead)
- `TimeSequenceColumn`, `TimeSecondsColumn`, and `TimeNanosColumn` classes (use `TimeColumn` instead)
- `new_recording()` function (use `RecordingStream()` constructor instead)
- `set_time_sequence()`, `set_time_seconds()`, and `set_time_nanos()` functions (use `set_time()` instead)
- `serve_web()` function (use combination of `serve_grpc()` and `serve_web_viewer()`)
- `AnyValues.with_field()` and `AnyValues.with_component()` methods (use `with_component_from_data()` and `with_component_override()` instead)
- Various deprecated methods on video and time-related components

If you encounter any errors related to missing methods or components, check if they were previously deprecated and update your code to use the recommended alternatives. The deprecation warnings from previous versions should have indicated the correct replacements to use.

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

## URDF loader: sending transform updates now requires `parent_frame` and `child_frame` fields to be set

Previous versions of the built-in [URDF](https://en.wikipedia.org/wiki/URDF) data-loader in Rerun required you to send transform updates with _implicit_ frame IDs, i.e. having to send each joint transform on a specific entity path.
Depending on the complexity of your robot model, this could quickly lead to long entity paths.
E.g. when you wanted to update a joint deeper in your model hierarchy.

In 0.28, this is now dropped in favor of transforms with _named_ frame IDs (`parent_frame`, `child_frame`).
This is more in line with the TF2 system in ROS and allows you to send all transform updates on one single entity (e.g. a `transforms` entity).

In particular, this results in two changes compared after you load an `URDF` model into Rerun compared to previous releases:

1. To update a joint with a `Transform3D`, the `parent_frame` and `child_frame` fields need to be set (analogous to how the joint is specified in the `URDF` file).
2. The transformation must have both rotation and translation (again, analogous to the `URDF`). Updating only the rotation is no longer supported.

For more details about loading & updating `URDF` models, we added a "Loading URDF models" page to our documentation in this release.

## Python SDK: catalog API overhaul

This release includes a major overhaul of the `rerun.catalog` module that aims to clarify and consolidate the APIs, and make them more future-proof. This includes improving naming, more consistently using DataFusion's dataframes, removing/merging redundant APIs, and exposing fewer implementation details.

We used deprecations to ease migration where possible, but several changes required breaking the API when deprecation would have been too complex. All deprecated APIs will be removed in a future release.

### "Partition" renamed to "Segment"

The term "partition" is overloaded in data science, and our use of it could be confusing. To avoid this, the "partition" terminology has been renamed to "segment" instead. The old APIs are deprecated.

| Old API                             | New API                           |
|-------------------------------------|-----------------------------------|
| `DatasetEntry.partition_ids()`      | `DatasetEntry.segment_ids()`      |
| `DatasetEntry.partition_table()`    | `DatasetEntry.segment_table()`    |
| `DatasetEntry.partition_url()`      | `DatasetEntry.segment_url()`      |
| `DatasetEntry.download_partition()` | `DatasetEntry.download_segment()` |
| `partition_url()`                   | `segment_url()`                   |
| `partition_url_udf()`               | `segment_url_udf()`               |

The column `rerun_partition_id` is now `rerun_segment_id` (breaking change), and the `partition_id` field on viewer event classes (`PlayEvent`, `PauseEvent`, etc.) is now `segment_id` (the old name is deprecated and will be removed in a future release).

### Catalog client

**Method renames** (deprecated, old names still work):

| Old API                              | New API                        |
|--------------------------------------|--------------------------------|
| `CatalogClient.all_entries()`        | `CatalogClient.entries()`      |
| `CatalogClient.dataset_entries()`    | `CatalogClient.datasets()`     |
| `CatalogClient.table_entries()`      | `CatalogClient.tables()`       |
| `CatalogClient.get_dataset_entry()`  | `CatalogClient.get_dataset()`  |
| `CatalogClient.get_table_entry()`    | `CatalogClient.get_table()`    |
| `CatalogClient.create_table_entry()` | `CatalogClient.create_table()` |

**New features:**
- `entries()`, `datasets()`, `tables()`, and their `*_names()` variants now accept `include_hidden=True` to include blueprint datasets and system tables.

**Breaking change:** The `entries()`, `datasets()`, and `tables()` methods now return lists of entry objects (`DatasetEntry`/`TableEntry`) instead of DataFrames. For DataFrame access to raw entries, use `client.get_table(name="__entries").reader()`.

### Tables

**Breaking changes:**

`get_table()` now returns a `TableEntry` object instead of a DataFrame:

```python
# Before (0.27)
df = client.get_table(name="my_table")

# After (0.28)
df = client.get_table(name="my_table").reader()
```

`TableEntry.df()` has been renamed to `TableEntry.reader()`.

**Deprecations:** Write operations moved from `CatalogClient` to `TableEntry`:

| Old API                                | New API                                                               |
|----------------------------------------|-----------------------------------------------------------------------|
| `client.write_table(name, data, mode)` | `table.append(data)` / `table.overwrite(data)` / `table.upsert(data)` |
| `client.append_to_table(name, data)`   | `table.append(data)`                                                  |
| `client.update_table(name, data)`      | `table.upsert(data)`                                                  |

The new methods also support keyword arguments: `table.append(col1=[1, 2, 3], col2=["a", "b", "c"])`.


### Dataset querying

**Breaking change:** `DataframeQueryView` has been removed. Its functionality has been split between `DatasetView` (for segment and content filtering) and standard DataFusion DataFrame operations (for row-level filtering).

Use `DatasetEntry.filter_segments()` and `DatasetEntry.filter_contents()` to create a `DatasetView`, then call `reader()` to get a `datafusion.DataFrame`. Any row-level filtering (like `filter_is_not_null()`) should now be done on the resulting DataFrame using DataFusion's filtering APIs.

```python
# Before (0.27)
view = dataset.dataframe_query_view(index="timeline", contents={"/points": ["Position2D"]})
df = view.filter_partition_id(["recording_0"]).df()

# After (0.28)
view = dataset.filter_segments(["recording_0"]).filter_contents(["/points/**"])
df = view.reader(index="timeline")
```

`DatasetEntry.segment_table()` and `DatasetEntry.manifest()` now return `datafusion.DataFrame` directly (no `.df()` call needed). `segment_table()` also accepts optional `join_meta` and `join_key` parameters for joining with external metadata.

Key migration patterns:
- Index selection: `dataset.reader(index="timeline")`
- Content filtering: `dataset.filter_contents(["/points/**"]).reader(...)` <!-- NOLINT -->
- Segment filtering: `dataset.filter_segments(["recording_0"]).reader(...)` <!-- NOLINT -->
- Latest-at fill: `dataset.reader(index="timeline", fill_latest_at=True)`
- Row filtering: Use DataFusion's `df.filter(col(...).is_not_null())` on the returned DataFrame <!-- NOLINT -->

The `DataFrame` created by `reader()` now supports server side filtering for segment IDs and time indices.
These can cause significant performance enhancements for some queries. Any filters involving these columns
should occur immediately after the creation of the `DataFrame` to ensure they are properly pushed down to
the server.

```python
df = view.reader(index="log_tick").filter(
    (col("rerun_segment_id") == "recording_0") & (col("log_tick") == 123456)
)
```

### Registration and tasks

The dataset segment registration APIs have been consolidated, and return a `RegistrationHandle` specific to the registration process. The more generic `Tasks` object previously used has been removed.

**Breaking change:** `register()` and `register_batch()` have been merged into a unified `register()` API that returns a `RegistrationHandle`:

```python
# Single registration
segment_id = dataset.register("s3://bucket/recording.rrd").wait().segment_ids[0]

# Batch registration
handle = dataset.register(["file:///uri1.rrd", "file:///uri2.rrd"], layer_name="base")
segment_ids = handle.wait().segment_ids

# Progress tracking
for result in handle.iter_results():
    print(f"Registered {result.uri} as {result.segment_id}")
```

The `recording_layer` parameter has been renamed to `layer_name`.

### Blueprints

The updated APIs are now abstracted from the underlying storage mechanism (blueprint datasets).

**Deprecations:**

| Old API                                             | New API                                |
|-----------------------------------------------------|----------------------------------------|
| `DatasetEntry.default_blueprint_partition_id()`     | `DatasetEntry.default_blueprint()`     |
| `DatasetEntry.set_default_blueprint_partition_id()` | `DatasetEntry.set_default_blueprint()` |

**New methods:**
- `dataset.register_blueprint(url, set_default=True)` - register and optionally set as default
- `dataset.blueprints()` - list all registered blueprints

### Search indexes

**Deprecations:** Methods renamed to clarify "search index" vs "dataset index":

| Old API                              | New API                                     |
|--------------------------------------|---------------------------------------------|
| `DatasetEntry.create_fts_index()`    | `DatasetEntry.create_fts_search_index()`    |
| `DatasetEntry.create_vector_index()` | `DatasetEntry.create_vector_search_index()` |
| `DatasetEntry.list_indexes()`        | `DatasetEntry.list_search_indexes()`        |
| `DatasetEntry.delete_indexes()`      | `DatasetEntry.delete_search_indexes()`      |

**Breaking change:** `search_fts()` and `search_vector()` now return `datafusion.DataFrame` directly:

```python
# Before (0.27)
result = dataset.search_fts("query", column).df()

# After (0.28)
result = dataset.search_fts("query", column)
```

### Schema types moved

The `Schema` class and column descriptor/selector types have moved from `rerun.dataframe` to `rerun.catalog`. The old import paths still work but are deprecated.

### Other deprecations

`DatasetEntry.download_segments()` is deprecated and will be removed in a future release.


## `RecordingView` and local dataframe API deprecated

With the OSS server and the catalog APIs gaining maturity, we want to make this the primary way to query data out of Rerun, including when working locally.
These APIs will receive ongoing improvements in the future, and offer a smoother migration path to cloud-based workflows.
As a result, we are deprecating `rerun.dataframe` in this release, and in particular the ability to run dataframe queries on a `Recording` object. See the following sections for more details.


### `RecordingView` deprecated


The `RecordingView` class, along with `Recording.view()` and the ability to run dataframe queries locally, is deprecated. Use `Server` and the `rerun.catalog` API instead for local dataframe queries. In addition, the `AnyColumn`, `AnyComponentColumn`, and `ViewContentsLike` helper types are deprecated.

**Before:**

```python
import rerun as rr

# Load a recording file
recording = rr.dataframe.load_recording("recording.rrd")

# Create a view and query
view = recording.view(index="frame_nr", contents="/world/**")
view = view.filter_range_sequence(0, 100)
view = view.fill_latest_at()

# Select columns and read data
batches = view.select()
table = batches.read_all()
df = table.to_pandas()
```

**After:**

```python
import rerun as rr
from datafusion import col

# Start a local server with the recording
with rr.server.Server(datasets={"my_dataset": ["recording.rrd"]}) as server:
    client = server.client()
    dataset = client.get_dataset("my_dataset")

    # Create a filtered view and query
    view = dataset.filter_contents(["/world/**"])
    df = view.reader(index="frame_nr", fill_latest_at=True)

    # Apply additional filters with DataFusion
    df = df.filter(col("frame_nr") <= 100)

    # Convert to pandas
    pandas_df = df.to_pandas()
```

For more details on the new API, see the [Query data out of Rerun](../../howto/get-data-out.md) guide.

### `Recording` moved to `rerun.recording`

The `Recording` class and recording loading functions have been moved to a new `rerun.recording`. The old import paths are deprecated.

| Old import                              | New import                              |
|-----------------------------------------|-----------------------------------------|
| `rr.dataframe.load_recording()`         | `rr.recording.load_recording()`         |
| `rr.dataframe.load_archive()`           | `rr.recording.load_archive()`           |
| `rr.dataframe.Recording`                | `rr.recording.Recording`                |
| `rr.dataframe.RRDArchive`               | `rr.recording.RRDArchive`               |

### `send_dataframe()` moved to top-level `rerun`

The `send_dataframe()` and `send_record_batch()` functions have been moved to the top-level `rerun` module and are also exposed as methods of `RecordingStream`. The old import paths are deprecated.

| Old import                              | New import                              |
|-----------------------------------------|-----------------------------------------|
| `rr.dataframe.send_dataframe()`         | `rr.send_dataframe()`                   |
| `rr.dataframe.send_record_batch()`      | `rr.send_record_batch()`                |

## `RecordingStream` now cleans up when going out of scope in Python SDK

`RecordingStream` objects are now cleaned up as they go out of scope. This specifically
includes flushing and closing the associated sinks.

This may lead to subtle changes in behavior if you are depending on side-effects of a
a non-global recording stream staying open. The most notable example is the `serve_grpc()`
sink. [See #12301](https://github.com/rerun-io/rerun/issues/12301) for an example and
context.

### Motivation
Consider an example like:
```python

def create_recording(data, filename):
    rec = rr.RecordingStream("rerun_example_cleanup")
    rec.save(filename)

    for event in data:
        rec.log(...)

def my_app():
    ...

    create_recording(data1, "data1.rrd")
    create_recording(data2, "data2.rrd")
    create_recording(data3, "data3.rrd")
```

**Before**
All 3 recording files would stay open and unterminated until the application exit.

**After**
Now each recording is closed and terminated incrementally as the functions return
and the RecordingStream objects go out of scope.
