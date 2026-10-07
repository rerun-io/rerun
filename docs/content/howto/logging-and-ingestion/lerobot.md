---
title: Working with LeRobot datasets
order: 750
description: Open and convert LeRobot datasets in Rerun
---

Rerun reads [LeRobot](https://huggingface.co/docs/lerobot/index) datasets, the directory-based format used for robot-learning datasets.
Both the v2.1 and v3.0 dataset layouts are supported.

## Viewing a LeRobot dataset

A LeRobot dataset is a directory (metadata, parquet, and video files), so point Rerun at the dataset directory:

```bash
rerun path/to/lerobot_dataset
```

You can also drag and drop the dataset directory into the Rerun Viewer, or load it using the SDK:

snippet: howto/load_lerobot

## Converting a LeRobot dataset to RRD

To query a LeRobot dataset on a [catalog server](../query-and-transform/get-data-out.md) or train on it with the Rerun [dataloader](../train/dataloader.md), convert each episode to its own `.rrd` file with [`LeRobotReader`](https://ref.rerun.io/docs/python/stable/experimental/#rerun.experimental.LeRobotReader):

```python
from pathlib import Path

import rerun as rr

Path("rrd").mkdir(exist_ok=True)
profile = rr.chunk.OptimizationProfile.OBJECT_STORE

reader = rr.experimental.LeRobotReader("path/to/lerobot_dataset")
for episode in reader.episodes():
    reader.stream(episode).collect(optimize=profile).write_rrd(
        f"rrd/episode_{episode}.rrd",
        application_id="my_dataset",
        recording_id=f"episode_{episode}",
    )
```

- The `recording_id` becomes the [segment](../../concepts/query-and-transform/catalog-object-model.md#datasets) ID once the file is registered, so pick one that names the episode.
- `collect(optimize=profile)` merges chunks into [larger ones for catalog queries](optimize-chunks.md#compacting-chunks-with-the-chunk-processing-api), at the cost of holding the episode in memory.
  Without it, `write_rrd` streams the episode straight to disk.
- In v3.0 datasets, video without B-frames, such as AV1 (LeRobot's default), is copied as-is.
  H.264 or H.265 with B-frames is re-encoded, and an episode that starts mid-GOP has its first GOP re-encoded.
  Both need `ffmpeg` on the `PATH`.
  In v2.1 datasets, each episode's video file is copied as-is into an `AssetVideo`, which the dataloader cannot decode.

Then load the folder into a local catalog with `rerun server -d my_dataset=rrd`, or register the files to an existing [dataset](../../concepts/query-and-transform/catalog-object-model.md#datasets).

## Data model

Each episode becomes its own [recording](../../concepts/logging-and-ingestion/recordings.md), and each feature becomes an [entity](../../concepts/logging-and-ingestion/entity-component.md) named after the feature key:

| LeRobot | Rerun |
| --- | --- |
| `frame_index` column | Sequence [timeline](../../concepts/logging-and-ingestion/timelines.md) `frame_index`. Datasets without it get a duration timeline `timestamp` instead |
| `float32` / `float64` feature, such as `observation.state` or `action` | [`Scalars`](../../reference/types/archetypes/scalars.md) at `/observation.state`. A vector feature is one list per row, and its `names` metadata becomes a static `SeriesLines:names` column |
| `video` feature | [`VideoStream`](../../reference/types/archetypes/video_stream.md) for v3.0 datasets, [`AssetVideo`](../../reference/types/archetypes/asset_video.md) with frame references for v2.1 |
| `image` feature | [`EncodedImage`](../../reference/types/archetypes/encoded_image.md) for 3-channel images, [`EncodedDepthImage`](../../reference/types/archetypes/encoded_depth_image.md) for single-channel images. Other channel counts are skipped with a warning |
| `task_index`, `subtask_index` | [`TextDocument`](../../reference/types/archetypes/text_document.md) at `/task` and `/subtask` holding the description |
| `string` feature | `TextDocument` at the feature's entity |
| `language` feature, such as `language_persistent` | `TextDocument` tracks under `/<feature>/<style>/<role>/<camera>`, leaving out any part that is missing, such as `/language_persistent/subtask/assistant`. Tool calls go to a `tool_calls` child entity |
| `episode_index`, `index`, and `timestamp` when `frame_index` exists | Not logged |
| `bool`, `int16`, and `int64` features, such as `next.done` or `next.success` | Not supported yet. Skipped with a warning |

Dots in a feature key are part of the entity name, not path separators: `observation.images.up` becomes the single entity `/observation.images.up`.
A [content filter](../../concepts/logging-and-ingestion/entity-path.md#entity-path-filters) such as `/observation.images.**` therefore matches nothing, so use the full entity name.

The dataset's frame rate is not stored in the recording.
To convert `frame_index` to seconds, divide by `fps` from the dataset's `meta/info.json`.

## Related

To go the other way — querying recordings and exporting them as a LeRobot dataset — see [Export recordings to LeRobot datasets](../train/lerobot_export.md).
