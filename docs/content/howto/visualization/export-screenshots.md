---
title: Export screenshots, videos, and point clouds
order: 130
description: Save or copy a screenshot of a view, take screenshots from code or the command line, and export data such as videos or point clouds
---

## Screenshot of a view in the Viewer

Right-click the title bar or tab of a view, or the view in the blueprint panel, and choose:

- "Copy screenshot" to copy it to the clipboard, or
- "Save screenshot…" to save it as a PNG file.

Copying screenshots is not supported in Safari and other WebKit browsers, including every browser on iOS.

## Screenshots from Python

`rerun.experimental.ViewerClient` can take screenshots of a running Viewer.
This API is experimental and may change.

```python
from rerun.experimental import ViewerClient

client = ViewerClient.connect()

# Save a PNG file on the machine that runs the Viewer:
client.save_screenshot("viewer.png")

# Get a PNG as bytes, without writing a file:
png_bytes = client.take_screenshot(max_size=1024)
```

Both methods take an optional `view_id` to capture only one view.
`client.get_state().views` lists the views and their IDs.

To take screenshots without a window, for example in CI, start a headless Viewer with `ViewerClient.spawn(headless=True)`.
This still needs a GPU or a software renderer.

## Screenshots from the command line

```sh
rerun recording.rrd --new --screenshot-to screenshot.png --window-size 1920x1080
```

The Viewer opens the file, saves a screenshot of the whole window, and quits.
`--new` starts a new Viewer.
Without it, if a Viewer is already running, the command only sends the recording to that Viewer and saves no screenshot.

## Export videos and point clouds

The Viewer cannot export a video of the playback, or export point clouds or meshes to files such as PLY.
See [missing features](../../development/roadmap.md#missing-features).

Workarounds:

- To make a video, take a screenshot per time step with `ViewerClient.set_time` and `save_screenshot`, and combine the images with a tool such as `ffmpeg`.
  You can also use a screen recorder.
- To export data, read it back with the [dataframe API](../../concepts/query-and-transform/dataframe-queries.md) and write the file yourself, for example a PLY file with `trimesh` or `open3d`.
