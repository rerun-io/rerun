---
title: "A blueprint's `TimePanel` no longer contains `play_state`"
hidden: true
type: breaking
---

### A blueprint's `TimePanel` no longer contains `play_state`

The play state is no longer part of the blueprint.
`TimePanel` in Python and Rust no longer takes a `play_state`, and the `TimePanelBlueprint` archetype no longer has a `play_state` field in any SDK.

Blueprints that still contain a play state open as before, and the Viewer ignores the value.

To pause, play or follow from code, use `ViewerClient.set_time` on a running Viewer:

```python
# Before
rr.send_blueprint(rrb.Blueprint(rrb.TimePanel(play_state="paused")))

# After
from rerun.experimental import ViewerClient

client = ViewerClient.connect()
client.set_time(play_state="paused")
```

See [Control playback and drive the Viewer from code](../viewer/extend/control-the-viewer.md).
`PreviewsConfig.play_state` for table previews is unchanged.
