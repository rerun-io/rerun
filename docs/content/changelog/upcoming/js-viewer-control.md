---
title: Typed viewer API in the JS web viewer
hidden: true
type: feature
---

### Typed viewer API in the JS web viewer

`WebViewer` has new typed methods to read and drive the viewer: `get_state`, `set_time`, `get_location`, and `set_location`.
Others open URLs and close recordings, read and write blueprints, read recording, dataset, and table schemas, inspect the transform graph, read the viewer's logs, run command palette commands, highlight a part of the screen, and take screenshots.

```ts
// Which recordings are open, and the time range of each of their timelines:
const state = await viewer.get_state();
for (const recording of state.recordings) {
  for (const timeline of recording.timelines) {
    console.log(recording.store_id, timeline.name, timeline.time_range);
  }
}

// Seek the active recording to frame 42, and play from there:
await viewer.set_time({ timeline: "frame", sequence: 42, play_state: "playing" });
```

The API is experimental.
The older methods for the active recording, timeline, time, time range, and play state (`get_active_recording_id`, `set_current_time`, `get_time_range`, `set_playing`, …) still work, but are deprecated in favor of it.

See the [`viewer_control` example](https://github.com/rerun-io/rerun/tree/main/examples/js/viewer_control) and [Embed a Rerun Viewer](../howto/integrations/embed-web.md).
