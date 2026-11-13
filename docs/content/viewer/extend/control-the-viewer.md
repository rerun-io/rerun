---
title: Control playback and drive the Viewer from code
order: 120
description: Set playback speed, timeline, and time cursor from code
---

## Configure playback in the blueprint

The time panel of the [blueprint](../blueprints-and-views/blueprints.md) controls playback:

```python
import rerun as rr
import rerun.blueprint as rrb

rr.send_blueprint(
    rrb.Blueprint(
        rrb.Spatial3DView(origin="/world"),
        rrb.TimePanel(
            timeline="frame",
            playback_speed=2.0,
            fps=30.0,
            play_state="playing",
            loop_mode="all",
        ),
    )
)
```

| Parameter        | Meaning                                                                   |
|------------------|---------------------------------------------------------------------------|
| `timeline`       | The active timeline.                                                      |
| `playback_speed` | Playback speed multiplier.                                                |
| `fps`            | Frames per second, for sequence timelines.                                |
| `play_state`     | `"paused"`, `"playing"`, or `"following"` (follow the newest data).       |
| `loop_mode`      | `"off"`, `"selection"`, or `"all"`. `"selection"` needs `time_selection`. |
| `time_selection` | A selected time range, as `rrb.components.AbsoluteTimeRange(min, max)`.   |
| `state`          | `"expanded"`, `"collapsed"`, or `"hidden"`.                               |

The time panel blueprint has no field for the position of the time cursor.
To start at a specific time, use one of the methods below.

## Open a recording at a specific time

A Viewer URL can end with a `#when=` fragment that sets the time cursor, for example `#when=frame@120`.
See [Viewer URLs](urls.md).

## Drive a running Viewer from Python

`rerun.experimental.ViewerClient` connects to a running Viewer and controls it.
This API is experimental and may change.

```python
from rerun.experimental import ViewerClient

client = ViewerClient.connect()  # rerun+http://127.0.0.1:9876/proxy by default
client.open("recording.rrd")
client.set_time("frame", sequence=120, play_state="paused")
client.set_time(play_state="playing")
client.save_screenshot("viewer.png")  # The path is on the machine that runs the Viewer
client.close("current")
```

Other useful methods:

- `ViewerClient.spawn(headless=True)` starts a new Viewer, for example for screenshots in CI.
- `get_state()` returns the open recordings, the time cursor, and the views.
  It does not include the current selection.
- `take_screenshot()` returns a PNG as bytes.
- `get_blueprint()` and `set_blueprint()` read and replace the blueprint as a JSON-like dictionary.

## In notebooks

The notebook `Viewer` has `set_time_ctrl`:

```python
from rerun.notebook import Viewer

viewer = Viewer()
viewer.display()
viewer.set_time_ctrl(timeline="frame", sequence=120, play=True)
```

## In the web Viewer

The [JavaScript `WebViewer`](../../integrations/embed-web.md#reading-and-driving-the-viewer) has `set_time`:

```ts
await viewer.set_time({ timeline: "frame", sequence: 42, play_state: "playing" });
```

## In the Viewer

- Right-click in a time series view to move the time cursor to that time.
- Press Space to play or pause.

## Open a file in an already running Viewer

`rerun recording.rrd` sends the file to a Viewer that is already running on the default port, instead of starting a new one.
To always start a new Viewer, use `rerun --new recording.rrd` (an alias for `--port auto`).
