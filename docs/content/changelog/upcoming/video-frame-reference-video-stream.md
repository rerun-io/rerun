---
title: "VideoFrameReference works with VideoStream"
hidden: true
type: feature
---

### `VideoFrameReference` works with `VideoStream`

[`VideoFrameReference`](../reference/types/archetypes/video_frame_reference.md) can now show frames from a [`VideoStream`](../reference/types/archetypes/video_stream.md), not just from an [`AssetVideo`](../reference/types/archetypes/asset_video.md).
As before, `video_reference` points at the entity holding the video, and defaults to the entity of the frame reference itself.

Frames of a `VideoStream` are looked up on the active timeline in the viewer, so the same stream can be shown at different timestamps in several views.

Docs: ../concepts/logging-and-ingestion/video.md
