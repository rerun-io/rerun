---
title: "0.43"
order: 940
---

## New features

### Chunk indexing policy in the Python catalog API

A dataset can now index only some of its entity paths and timelines, so registration and queries skip the data you do not need.
Pass a `rerun.catalog.ChunkIndexingPolicy` to `CatalogClient.create_dataset(chunk_indexing_policy=…)`, read it back with `DatasetEntry.chunk_indexing_policy`, and change it with `DatasetEntry.set_chunk_indexing_policy`.
Changing it re-indexes the registered data in the background; call `wait()` on the returned `rerun.catalog.DatasetPolicyHandle` to block until that finishes.
Each dimension is a list of names, or `"all"` for every name, including names logged later.
Resolve entity path rules such as `/world/**` against the dataset schema with `ChunkIndexingPolicy.match` before sending them.
Without a policy, a dataset indexes everything, and `DatasetEntry.set_chunk_indexing_policy(None)` resets it to that.
A query in the indexed scope returns only the indexed entity paths, and raises `rerun.catalog.ChunkNotIndexedError` if it names an entity path or timeline the policy excludes.
A query in the full scope, the default, reads excluded data from the recordings themselves, which works only for specific segments; otherwise it raises `ChunkNotIndexedError` too.

Only Rerun Hub applies the policy; the OSS server stores it and ignores it.

### Dataset scope for Python queries

`DatasetEntry.reader` and `DatasetSelection.reader` take a `scope=` argument of type `rerun.catalog.DatasetQueryScope`, which selects the data a query reads relative to the dataset's chunk indexing policy.
On Rerun Hub, `scope="indexed"` reads only indexed data: selecting every entity path selects only the indexed ones, and naming an entity path or timeline the policy excludes raises `rerun.catalog.ChunkNotIndexedError`.
`scope="full"`, the default, covers everything.
On Rerun Hub, it reads data outside the policy from the recordings themselves.
That works only for specific segments, so filter the dataset to them first.
Without them, such a read raises `rerun.catalog.ChunkNotIndexedError`.

`DatasetEntry.schema`, `DatasetSelection.schema` and `DatasetSelection.arrow_schema` also take `scope=`.
The OSS server ignores the scope.

## Other

- **Blueprint edits are remembered per blueprint**: The native viewer now remembers your edits to a dataset's default blueprint separately from your edits to other blueprints, so switching between them keeps both sets of changes. A blueprint you haven't edited shows the version the server had when you first opened a segment of the dataset. All segments of a dataset share that blueprint. Blueprint edits saved by earlier versions of the viewer are kept.
- **Pan and zoom in the tensor view**: The [tensor view](../reference/types/views/tensor_view.md) now supports pan and zoom, with the same controls as the 2D view: drag or scroll to pan, zoom with the scroll modifier or a pinch, and double-click to reset. The visible region is stored in the new `visual_bounds` view property, so it can also be set from the SDK. It is reset to the whole slice when the dimensions shown as width or height change, or when the view fit changes.

  <video width="100%" autoplay loop muted controls>
      <source src="https://static.rerun.io/ea60f9e014adbbbcbec191d62bd096cd9599a7a4_tensor_view_zoom_viewer.mp4" type="video/mp4" />
  </video>
- **Stay on the current recording while new ones stream in**: With "Switch to newly streamed recordings" turned off in the settings, a recording that streams in from an SDK is added to the recordings panel instead of replacing the one you are looking at.
- **Edit grid and volume placement and voxel size in the viewer**: You can now edit the translation of `GridMap`, `VoxelGridMap`, and `Volume3D` in the viewer, and adjust voxel size for `VoxelGridMap` and `Volume3D`.
- **Experimental: read the selection over ViewerControl**: `ViewerClient.get_selection()` in Python, `WebViewer.getSelection()` in JavaScript, and the `rerun_get_selection` MCP tool report what the user has selected in the Viewer: entities and instances, components, views, containers, and recordings. `ViewerClient` is part of `rerun.experimental` and may change in future releases.
- **`rerun.catalog.TimelineKind`**: `TimelineInfo.kind` is now a `rerun.catalog.TimelineKind` enum, which maps to and from the Arrow datatype of the timeline's index column with `kind.datatype` and `TimelineKind.from_datatype(…)`. It is a string enum, so comparisons such as `kind == "sequence"` keep working.
- **Memory limits respect cgroup limits on Linux**: On Linux, the default memory limits and percentage limits such as `--memory-limit 50%` are now a fraction of the process's cgroup memory limit, rather than full machine memory. This is particularly useful when running inside of a container.

## Breaking changes

### JavaScript API uses camelCase <!-- NOLINT -->

The JavaScript viewer API now uses camelCase names.
Previously released snake_case methods, options, and returned fields remain available as deprecated aliases.
These aliases will be removed in a future release.

Before:

```js
await viewer.set_time({ play_state: "paused" });
const recording = (await viewer.get_state()).active_store_id;
```

After:

```js
await viewer.setTime({ playState: "paused" });
const recording = (await viewer.getState()).activeStoreId;
```

### `object_store_config` renamed to `storage_location`

In `DatasetEntry.register()`, replace `object_store_config` with `storage_location`; the old keyword still works with a deprecation warning.
Supplying both keywords raises `ValueError`, even if their values match.
Invalid storage location aliases now raise `ValueError`.

---

Looking for an older release? See the [migration guides for 0.33 and earlier](../reference/migration.md).
