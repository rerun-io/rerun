---
title: "`ViewerClient.set_time` takes a `play_state`"
hidden: true
type: breaking
---

### `ViewerClient.set_time` takes a `play_state`

In `rerun.experimental.ViewerClient.set_time`, the `play: bool` argument is replaced by `play_state`, which is `"paused"`, `"playing"`, `"following"`, or `None`.
If you omit it, the viewer now keeps playing or stays paused as it was, instead of pausing.
The time is now optional too, so you can switch timelines, or play, pause, or follow the latest data, without moving the time cursor.

```python
# Before
viewer.set_time("frame", sequence=42)  # paused
viewer.set_time("frame", sequence=42, play=True)

# After
viewer.set_time("frame", sequence=42, play_state="paused")
viewer.set_time("frame", sequence=42, play_state="playing")
viewer.set_time(play_state="following")
```
