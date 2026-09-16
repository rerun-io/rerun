---
title: "`ViewerState` reports what is still loading"
hidden: true
type: feature
---

### `ViewerState` reports what is still loading

`GetViewerState` — and with it the `rerun_get_viewer_state` MCP tool and `ViewerClient.viewer_state()` — now lists the data sources the Viewer is still loading from.

```python
state = client.viewer_state()
while state.loading:
    print(state.loading[0].status)  # "Loading /path/to/dataset…"
    state = client.viewer_state()
```

`open_url` returns as soon as a load *starts*, and a recording shows up as soon as its first message lands, so this is how a caller tells a recording that is still arriving from one that arrived empty.
Sources that only wait for someone else to send data, such as an SDK connection, are left out: they never finish.

See the [MCP reference](../reference/viewer/mcp.md).
