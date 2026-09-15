---
title: "`LeRobotReader`: stream LeRobot datasets as lazy chunk streams"
hidden: true
type: feature
---

### `LeRobotReader`: stream LeRobot datasets as lazy chunk streams

`rerun.experimental.LeRobotReader` reads a LeRobot dataset (v2 or v3) one episode at a time:

```python
reader = rr.experimental.LeRobotReader("path/to/dataset")
for episode in reader.episodes():
    reader.stream(episode).write_rrd(
        f"episode_{episode}.rrd",
        application_id="my_dataset",
        recording_id=f"episode_{episode}",
    )
```

Streaming is lazy end to end: memory is bounded by chunk size, not by episode or dataset size.
Videos are cut to the episode's time window.
B-frame-free video (AV1, LeRobot's default codec) streams directly; a stream that must be re-encoded — H.264 with B-frames, or a window starting mid-GOP — needs ffmpeg on the `PATH`.
