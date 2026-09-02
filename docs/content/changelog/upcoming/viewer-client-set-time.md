---
title: Control the Viewer time cursor from Python
hidden: true
type: feature
---

### Control the Viewer time cursor from Python

The experimental Python [`ViewerClient`](https://ref.rerun.io/docs/python/main/experimental/#rerun.experimental.ViewerClient) can now seek the Viewer's active recording to a sequence, duration, or timestamp and optionally start playback.
Connect to a running Viewer, then specify a timeline or omit it to use the active timeline:

```python
from rerun.experimental import ViewerClient

viewer = ViewerClient.connect()
viewer.set_time("frame", sequence=42)
viewer.set_time(duration=1.5, play=True)
```
