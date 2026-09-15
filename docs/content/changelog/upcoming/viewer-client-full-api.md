---
title: Drive a running Viewer from Python
hidden: true
type: feature
---

### Drive a running Viewer from Python

`rr.experimental.ViewerClient` now covers the whole viewer-control API, not just screenshots and the time cursor.

```py
import rerun.experimental as rre

client = rre.ViewerClient.connect("rerun+http://127.0.0.1:9876/proxy")

state = client.viewer_state()
print(state.catalog_url, [r.store_id for r in state.recordings])

client.open_url("/path/to/recording.rrd")
client.set_time("frame", sequence=42)

for entry in client.viewer_logs():
    print(entry.level, entry.message)

client.close_recordings(state.recordings[0].store_id)  # or "current", or "all"
```

`viewer_state` reports what the Viewer is showing: the open recordings with their timelines and time ranges, the current time cursor, and the views of the current blueprint together with the warnings and errors each one reports.
It also reports the Viewer's own catalog address, which you can hand to `CatalogClient` to read the data behind those recordings.

See the [Viewer control reference](../reference/viewer/mcp.md) for the same operations driven by an agent over MCP.
