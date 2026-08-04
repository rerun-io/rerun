---
title: "0.34"
order: 976
---

<!--
Release changeset for 0.34, reconstructed from the shipped release notes after
the fact (this release predates the per-PR `upcoming/` workflow). Its features
are listed under Highlights, as they were in the original release notes, so
there is no separate "New features" section.
-->

## Highlights

### Viewer MCP

We've added a MCP that allows an llm agent to see and interact with the Viewer!
You could ask your agent to
 - verify its work looks as expected in the Viewer.
 - debug a bug when something doesn't show up right.
 - explore a recording or dataset to search for specific patterns.

The agent has full control over the Viewer, meaning it can see and click any widget.

Here's an example where Claude Sonnet was asked to create a fancy particle animation of the Rerun logo and verify its
work using the mcp in the open Viewer (sped up by a lot, except when showing the end result):

https://github.com/user-attachments/assets/14ffe7ed-6000-4193-900c-627784682125

Once it wrote the script, it logged the recording to the Viewer, and then iterated until the result looked as requested.
It adjusted the camera position, improved the particle rendering by looking at different frames in the animation,
and then debugged why the fade out animation was still showing particles on the last frame.

<details>
  <summary>Full prompt</summary>

  > /goal Create a new rerun python example in this folder that uses reruns 2D shapes to recreate the rerun logo (rerun-wordmark-black.svg).
  > There should be a nice fade-in animation in the beginning, 10 frames duration. Then pause a bit with the full rerun logo visible and then
  > the shapes should explosively fade away with a 20 frame animation before the recording ends.
  >
  > You may only stop once the recreated logo in the viewer looks close to the provided svg (black text, white background).
  > Use the mcp to verify in the open viewer, don't ever kill it. Once done, launch an opus agent and ask it to judge how
  > closely it looks to the original image. Keep going until it's convinced that it looks close.
</details>

See our [mcp docs](https://rerun.io/docs/reference/viewer/mcp) to get started.

### Learning course

https://rerun.io/learn is a great way to learn how the Rerun data model covers the full physical AI experiment loop.
It is a short, hands-on course for robotics ML engineers who want the full robot learning data loop in one place:
```
raw data -> RRD -> derived layers -> dataset queries -> training -> evaluation
```

### Rerun agent skills

We added new skills to the Rerun repo to make it easier to investigate existing robotics data with Rerun.
You can install the skills in your project via:
```sh
npx skills add rerun-io/rerun
```

The new [learning course](https://rerun.io/learn) also shows how these agent skills can be used to collect, refine and train with robotics data.

### `VoxelGridMap` archetype

Rerun now supports sparse voxel grids through a new [`VoxelGridMap`](https://rerun.io/docs/reference/types/archetypes/voxel_grid_map) archetype (thanks to [@makeecat](https://github.com/makeecat) for the contribution!).
The archetype supports sparse indexing, anisotropic voxel sizes, pose offsets, and optional explicit colors or values & colormap per voxel.

Rerun's MCAP importer now also converts the *dense* ROS `nav2_msgs/VoxelGrid` and Foxglove `VoxelGrid` formats to Rerun `VoxelGridMap`.

And if you wonder how the smooth 3D navigation through the [voxel scene](https://github.com/ephtracy/voxel-model) in this video was done, see below!

<!-- https://static.rerun.io/7724132292eb25c643530304c6699270aeaa68e1_voxel_grid_teaser.mp4 -->
https://github.com/user-attachments/assets/87fb80da-66dd-4fcd-8b35-ab553696f536

### 🎮 Gamepad support in 3D views

You can now use a gamepad to navigate 3D views in the native viewer.
This makes it easier to do fine-grained, complex maneuvers with varying speed - e.g. for navigating large scenes or for screen videos.
Analog sticks control the eye position and look target, shoulder triggers move the eye up and down, and shoulder buttons accelerate/decelerate.

**Note:** The gamepad feature is currently experimental and can be activated through the settings menu.
Switch the 3D view's eye controls to `FirstPerson` for optimal experience.
Under the hood, we use the [`gilrs`](https://crates.io/crates/gilrs) crate that supports a wide range of devices.

### Drag & drop components

You can now drag & drop a component right from the streams panel to visualize it in a Time series view or Status timeline.

<!-- https://static.rerun.io/95f484cd8a2e937acd2eafa424bc778fe3ef5d7b_615146790-591024b9-57e7-4864-98f6-0b15ffb7ca2b-1782828747052.mp4 -->

https://github.com/user-attachments/assets/d70587a9-2020-4ae8-9cf3-0fef54dcf896

### Transform debugging tool

We added a new debugging UI for visual introspection of the 3D transform cache.
This allows to view the tree structure of the transform hierarchy, including potentially disconnected trees, and inspect the latest stored values of each frame node or transform edge.
The UI supports horizontal and vertical tree layout and you can filter by transform type (e.g. static or temporal).

**Note:** this UI is currently a tab in the dev panel (accessible via "Toggle dev panel" in the menu or ctrl/cmd+shift+m).
But we are open to making this a dedicated view in the future - let us know if you have any feedback!

<!-- https://static.rerun.io/cc6c41138eeeabb31fb2ec988eefdcd8da446c86_transform_dev_panel_teaser.mp4 -->
https://github.com/user-attachments/assets/b4b1ea6e-bce9-4e88-9ede-262f545e3b47

## Breaking changes

### `log_tick` no longer logged by default; `log_time` can be disabled

The SDK no longer injects the `log_tick` timeline column into logged data by default.
The `log_time` timeline is still injected by default, but can now be disabled.

The initial defaults are controlled by environment variables, read once on first use:

| Variable         | Default | Effect                                                            |
|------------------|---------|-------------------------------------------------------------------|
| `RERUN_LOG_TICK` | off     | Set truthy (`1`/`true`/`on`/…) to inject the `log_tick` timeline. |
| `RERUN_LOG_TIME` | on      | Set falsy (`0`/`false`/`off`/…) to skip the `log_time` timeline.  |

They can also be toggled at runtime, either on the active recording or on a specific `RecordingStream`:

snippet: migration/log_tick_enabled

If you relied on the `log_tick` timeline being present, set `RERUN_LOG_TICK=1` (or call `set_log_tick_enabled(true)`) to restore the old behavior.

### `rerun.recording` module removed

The `rerun.recording` module — `Recording`, `RRDArchive`, `load_recording`, `load_archive` — has been removed, having been deprecated in 0.32.
The related `rr.send_recording()`, `RecordingStream.send_recording()`, `Recording.from_chunks()`, and `DatasetEntry.download_segment()` are removed as well.

Use `rerun.experimental.RrdReader` instead.
See the [0.32 migration guide](../reference/migration/migration-0-32.md#rerunrecording-deprecated-in-favor-of-rrdreader) for more details.

### Remove embedded base64-encoded table blueprints & replace with blueprint registration

Table blueprints are no longer read from the Arrow schema metadata key `rerun:table_blueprint`.
If you previously stored `base64:…` encoded `.rbl` bytes in table metadata, export that blueprint as a regular `.rbl` file and register it with `TableEntry.register_blueprint(...)` instead.
Tables without a registered blueprint fall back to Arrow field metadata and viewer heuristics.

> [!NOTE]
> As of this release table blueprints alongside dataset preview are still regarded as an
> experimental feature which means that the table & APIs for table blueprints may change significantly.

### `DatasetEntry.manifest()` deprecated

`DatasetEntry.manifest()` was always intended for internal and debugging use only and should never have been part of the public API.
It is now marked `@deprecated` and will be removed in a future release.
No public replacement is offered.

### Remove previously deprecated SDK methods for custom indices

The `DatasetEntry` methods `create_fts_search_index`,  `create_vector_search_index`, `delete_search_indexes`, `search_fts`, and `search_vector` have been removed, having been deprecated in 0.31.

This change does not impact your ability to search through your dataset via [dataframe queries](https://rerun.io/docs/concepts/query-and-transform/dataframe-queries).

### `rr.send_dataframe` is now stricter and built on `Chunk.from_record_batch`

`rr.send_dataframe` / `rr.send_record_batch` are now thin wrappers over the new `rerun.experimental.Chunk.from_record_batch` (and `Chunk.from_dataframe`), which turns an Arrow record batch into one chunk per entity path.
This makes the Arrow → chunk interpretation a first-class, well-specified capability, but it changes a few behaviors that previously happened silently.

Consequently, the following breaking behavior changes are introduced:

- A batch with no index column now raises instead of silently logging static data.
  Opt in for a static chunk explicitly with `index=None` for static, or specify a column to use as index with `index=<column>` for temporal chunk.
- Entity-path recognition from a column name now requires a leading `/`.
  Names without it are no longer parsed for an entity path: `foo` and `foo:bar` previously became the entity `/foo`, and now land on the root entity `/` as components.
  Only `/entity:component` names are split.
  (Column names emitted by the Rerun SDK always have the `/` prefix.)
- As a consequence, `property:…` columns now land on the root entity `/` rather than an entity named `property`.
  Neither map back to `/__properties` — proper handling of this is not yet implemented.
- `component_type` is no longer defaulted to the literal `"Unknown"` when absent; it is left unset.

### `ParquetReader` column rules removed in favor of lenses

`rerun.experimental.ParquetReader` no longer accepts the `column_rules` parameter, and the `ColumnRule` class has been removed.
`ParquetReader` is now a pure reader — it turns raw parquet columns into grouped, time-indexed chunks of struct/scalar components.
Mapping those struct fields into Rerun archetypes is now done with lenses on the reader's `.stream()`.

### `SaveScreenshot` gRPC endpoint moved to new `ViewerControlService`

Previously, the `SaveScreenshot` gRPC endpoint was part of the `MessageProxyService`.

---

Looking for an older release? See the [migration guides for 0.33 and earlier](../reference/migration.md).
