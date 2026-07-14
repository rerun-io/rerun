---
name: rerun-data-model
description: "How raw multimodal robot data maps onto the Rerun data model. Read FIRST, before modeling or converting a dataset — and whenever you are about to convert/ingest/preprocess robot data into an .rrd or build a Rerun recording, even if not asked for the data model. Resolves the entity-vs-component, property-vs-component-vs-layer, and static-vs-temporal decisions and routes to the mechanism (do it with readers and lenses, not hand-built chunks or per-message rr.log): rerun-chunk-processing and the importer skills rerun-mcap, rerun-urdf, rerun-parquet, rerun-lerobot."
user_invocable: true
allowed-tools: Read, Grep, Bash, WebFetch
---

# Rerun data model

The hard part of ingesting a dataset is the **modeling decision**, not the API call.
Get the model right and any mechanism works; get it wrong and queries, views, and training all break.
This skill is just the decisions.
For mechanism details see `rerun-chunk-processing` (pipeline mechanics) and the importer skills it routes to: `rerun-mcap`,`rerun-urdf`,`rerun-parquet`, `rerun-lerobot`.
For exact signatures, the docs at `rerun.io/docs/concepts/logging-and-ingestion`.

**Before writing conversion code, fill in the mapping table below.** It is the design, and a human can review it in seconds.

## Pick the mechanism before you model the bytes

Modeling decides _what_ each datum becomes; this decides _how_ it gets there — and the default is **a reader + lenses, not hand-built chunks**:

- **Does a reader exist for this source?** MCAP→`McapReader`, URDF→`UrdfTree`, parquet→`ParquetReader`, RRD→`RrdReader`, LeRobot dir→`log_file_from_path`. **Yes →** `reader.stream()` + lenses; the reader produces the chunks, you do not.
- **No, and it is genuine external metadata** (JSON calibration, offsets) or a specific custom use case that can't be covered by a generic reader**→** `Chunk.from_columns`.
- **Otherwise**: consider if you are about to hand-build something a reader or lens should produce and ask for clarification.

For MCAP specifically: a Foxglove- or ROS-decodable file emits archetypes like `Transform3D`, `Pinhole`, and `VideoStream` for certain supported message schemas ready-made — pass those through, never re-derive them; only custom protobuf signal topics need lenses. The full decision tree and the anti-pattern list are in `rerun-chunk-processing`.

## The model

```
Dataset → Segment → Layer → Recording → Entity → Component → Chunk
```

- **Entity** = a thing, named by a path (`/robot/arm/camera`).
  **Component** = one typed field on it (positions, image, a scalar).
  **Archetype** = a standard bundle of components that log and render together (`Points3D`, `Image`, `Transform3D`).
- Every value you log sits on two axes: **where** (entity path + component) and **when** (which timeline, or _static_ = all time).
- **Segment** = one episode; on registration its `recording_id` becomes the `segment_id`. **Layer** = an extra `.rrd` on top of a segment. It attaches by matching the segment's `recording_id` and nothing else: that shared id is the whole layering mechanism.

## The decisions

**Entity vs component on an existing entity**
New entity if it has its own spatial frame (`Transform3D`/`Pinhole`), its own annotation context, or should be shown/cleared/shared independently (each robot link, each camera, each sensor). Same entity + extra component for auxiliary data at the same instances (per-point confidence).
Use `AnyValues` for non-standard fields.

**Property vs component vs layer** (the most common confusion)

- **Component**: per-timestamp signal on the timeline (joint angle, image).
- **Segment property**: one-per-episode metadata for catalog filter/search (operator, robot, site, task, date). Not per timestamp.
- **Layer**: a whole derived `.rrd` over a base segment (FK transforms, point clouds, gripper state, labels, quality scores). Queryable as if part of base.

**Static vs temporal**
Static belongs to all timelines and shadows any temporal value of the same component on the same entity. Use it for invariants (calibration, coordinate frames, robot meshes, annotation context, a video asset). Never make per-frame data static.

**Which timeline**
A `timestamp` timeline (ns since epoch) for cross-sensor clock alignment, a `sequence` timeline for frame/ordinal alignment; stamp on both when useful.
**Do not resample to a common rate.** Latest-at reconciles multi-rate streams at query time by holding each component's last sample (no interpolation).

**Base vs layer**
Base = faithful conversion of the raw streams, nothing computed.
Layer = anything derived (FK from URDF + joint states, clouds from depth + intrinsics).
Keep them separate `.rrd`s.

## The mapping table (produce before coding)

| Source (topic/column/key) | Entity path          | Archetype                             | Component(s)         | Timeline      | Static/temporal             | Base/layer | Property? |
| ------------------------- | -------------------- | ------------------------------------- | -------------------- | ------------- | --------------------------- | ---------- | --------- |
| mcap `/joint_states`      | `/robot/<joint>`     | `Scalars`                             | `scalars`            | `sensor_time` | temporal                    | base       | no        |
| `cam0/color.mp4`          | `/camera/cam0/video` | `AssetVideo`+`VideoFrameReference`    | asset+refs           | `video_time`  | asset static, refs temporal | base       | no        |
| `calibration.json`        | `/camera/cam0`       | `Pinhole` (+`Transform3D` extrinsics) | `image_from_camera`  | —             | static                      | base       | no        |
| URDF + joints (computed)  | `/robot/<link>`      | `Transform3D`                         | translation/rotation | `sensor_time` | temporal                    | **layer**  | no        |
| `episode.json` operator   | segment              | —                                     | —                    | —             | —                           | —          | **yes**   |

## Patterns worth knowing

- **Transforms / FK trees**: log a `Transform3D` per link entity; it relates to the **parent path** and composes down the tree. (Named `CoordinateFrame` + `child_frame`/`parent_frame` only when topology must be a flexible graph.)
- **Cameras**: extrinsics (`Transform3D`) + intrinsics (`Pinhole`) on the camera entity, image/depth as **children** so they inherit the projection.
- **Video**: `VideoStream` for raw H.264/H.265 samples.
- **Columnar ingest**: for an existing _file_, use the matching reader (`rerun-mcap`/`-parquet`/`-lerobot`), which produces chunks directly — do not hand-assemble `send_columns` from a custom parser. When you do log columns directly (live logging), `send_columns` adds **no** automatic timelines, so pass every timeline you want.

## Gotchas that cause real failures

1. Component columns come back as `ListArray` in queries: index `[0]`/`[0][0]` (0-based DataFrame, 1-based SQL). See `rerun-catalog-queries`.
2. A layer must share the segment's `recording_id`, or it won't attach. `application_id` is discarded on registration.
3. `send_columns`/`send_chunks` add no `log_time`/`log_tick`; only the timelines you pass exist.
4. Static shadows all temporal data of that component for all time; static is overwritten in the viewer but every write stays on disk until `rerun rrd optimize`.
5. One `Transform3D`/`Pinhole` relation per frame pair; logging the same relation on a second entity is rejected.
6. Entity paths are not file paths (`..` is meaningless, `__` is reserved).
7. A catalog **layer** written with a default `RecordingStream` injects a `/__properties` (`RecordingInfo`) chunk that can collide with the base segment's properties when the catalog merges layers by `recording_id`; the dataset's default blueprint then stops applying on open. Construct the layer's stream with `send_properties=False`.
