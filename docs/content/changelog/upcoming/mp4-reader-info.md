---
title: "Inspect an MP4 file without streaming it"
hidden: true
type: misc
---

### Inspect an MP4 file without streaming it

`rerun.experimental.Mp4Reader.info()` returns an `Mp4Info` with the codec, dimensions, frame count, GOP sizes, and a per-frame Arrow table of the file, read from the container metadata alone.
`Mp4Info.keyframe_timestamps_ns` lists the keyframes without streaming the video, and without FFmpeg for sources with B-frames.
