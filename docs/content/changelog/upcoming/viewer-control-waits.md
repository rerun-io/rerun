---
title: "Viewer-control operations answer once they are done"
hidden: true
type: misc
---

### Viewer-control operations answer once they are done

`open_url`, `close_recordings`, `set_time_cursor`, `set_blueprint`, and `run_command` now answer once the Viewer has carried them out, so the next `viewer_state()` sees the result.
`open_url` waits a few seconds at most for what it loads, and `ViewerClient.open_url` returns `False` if it was still loading by then.
See the [MCP reference](../reference/viewer/mcp.md).
