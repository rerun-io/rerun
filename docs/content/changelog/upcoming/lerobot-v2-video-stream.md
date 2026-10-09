---
title: LeRobot v2.1 videos load as `VideoStream`
hidden: true
type: misc
---

### LeRobot v2.1 videos load as `VideoStream`

[LeRobot](../howto/logging-and-ingestion/lerobot.md) v2.1 datasets now load video as `VideoStream` instead of `AssetVideo`, like v3.0.
Video with B-frames, such as most H.264, now needs `ffmpeg` on the `PATH`, and the re-encode can make ingesting these datasets slower.
