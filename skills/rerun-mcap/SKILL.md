---
name: rerun-mcap
description: Ingest MCAP files into Rerun chunk streams with rerun.experimental.McapReader. Read when converting an MCAP recording, selecting topics or decoders, decoding custom protobuf messages, or when an MCAP-derived stream comes out empty. Builds on rerun-chunk-processing (stream mechanics) and rerun-data-model (what the topics should become).
user_invocable: true
allowed-tools: Read, Grep, Bash, WebFetch
---

# Rerun MCAP ingestion

`McapReader` turns an MCAP file into a lazy chunk stream: one entity per topic
at the topic's path, message payloads decoded by pluggable decoders. This
skill covers the reader's options, what each topic becomes, and the failure
modes that yield an empty stream with no error. Stream mechanics (filter, drop,
lenses, merge, write) are in `rerun-chunk-processing`.

## The API

```python
from rerun.experimental import McapReader

reader = McapReader(mcap_path)  # see help(McapReader) for the full option set
stream = reader.stream()
```

A URDF embedded in the MCAP can be ingested as well (then see `rerun-urdf`).

## What a topic becomes

With the default decoders (`decoders=None`), the message **schema name** decides what a topic becomes — **pass archetypes through, lens only the raw `:message` topics**:

| MCAP schema name | decodes to | what to do |
| --- | --- | --- |
| `foxglove.FrameTransforms` | `Transform3D` | pass through; do **not** hand-build |
| `foxglove.CameraCalibration` | `Pinhole` | pass through |
| `foxglove.CompressedVideo` | `VideoStream` (real sample bytes) + `CoordinateFrame` | pass through |
| other `foxglove.*` well-known types | the matching archetype | pass through |
| ros2 well-known types (`ros2msg` / `ros2_reflection`) | archetype | pass through |
| your own `schemas.proto.*` / custom protobuf | one `<schema.name>:message` struct | attach semantics with a `DeriveLens` + `Selector` |

So a camera topic already arrives as `Pinhole`, its video as `VideoStream`, and a `frame_transforms` topic as `Transform3D` — only custom messages (e.g. a custom joint states schema, a custom gripper status enum) come through reflection or raw only and need lenses.

The `foxglove` decoder does the schema→archetype mapping; because foxglove messages are protobuf-_encoded_ it rides on the `protobuf` decoder, so keep `decoders=None` (verified: `decoders=["protobuf"]` alone leaves `foxglove.CameraCalibration` a raw `:message`; adding `foxglove` makes it a `Pinhole`). Confirm on your file: `McapReader(path).stream()`, then read `McapSchema:name` and a few `Chunk.format()` before deciding anything is missing or needs rebuilding.

- Entity path = topic name (`/sensors/joint_states` stays `/sensors/joint_states`).
  Filter early: `McapReader(path).stream().filter(content="/sensors/**")`.
- A reflection-decoded message lands as one struct component named `<fully.qualified.MessageName>:message`.
  Navigate it with `Selector` (`Selector(".joint_positions")`) inside lenses; this is how custom messages
  get Rerun semantics attached (see the DeriveLens patterns in `rerun-chunk-processing`).
- Topic regexes use RE2 syntax and are **not anchored**: `cam` matches
  `/external/cam_low` and `/camera_info`.
  Anchor explicitly (`^/external/cam`) when it matters. Prefer reader-level topic filtering over `.filter(...)`
  when you can, so excluded topics are never decoded at all.

## When to use the low-level `mcap` package instead

`McapReader` keeps payloads in columnar chunk streams; that is almost always what you want.
Drop to `mcap.reader.make_reader` only when you need raw record metadata without payloads, or when you need to rewrite the container itself (re-registering schemas, channels, and messages).

## Gotchas

1. Empty stream, no error: a topic regex that matched nothing, or a channel
   whose decoder produced no rows. Check `Chunk.format()` on a few chunks of
   `reader.stream().to_chunks()` against a tiny test file, or compare topic
   names with the `mcap` CLI / package first.
2. Topic regexes are unanchored RE2; excludes run after includes.
3. `timeline_type="timestamp"` interprets MCAP log times as wall-clock ns
   since epoch. If the recording's clock is wrong, fix it at the reader with
   `timestamp_offset_ns` rather than mutating timestamps downstream.
4. Decoder subsets silently skip topics no decoder claims; when a topic is
   missing, retry with `decoders=None` to rule out decoder selection.
5. Example fix-lenses are dataset-specific. Before copying a `MutateLens` like
   the `Pinhole:resolution` swap from the `robot_data_preprocessing` example,
   read the raw component from `McapReader(path).stream()` and confirm the defect
   exists in _your_ data — applied blindly it corrupts correct calibration (a
   correct 648×480 flipped to 480×648).
6. `foxglove` derives both the camera's `Pinhole:child_frame` and the video's
   `CoordinateFrame:frame` from each message's `.frame_id` (plus an image-plane
   suffix), so they **match** when the calibration and video topics share a
   `frame_id`. Only when those topics carry _different_ `frame_id`s does the
   video frame diverge and orphan the video from its image plane — re-home it
   then with a per-camera `MutateLens` on `CoordinateFrame:frame`.

## References

- End-to-end MCAP pipeline: `https://github.com/rerun-io/rerun/tree/main/examples/python/robot_data_preprocessing`
- `rerun-chunk-processing` (stream/lens mechanics), `rerun-urdf` (FK from joint-state topics), `rerun-data-model` (modeling decisions)
