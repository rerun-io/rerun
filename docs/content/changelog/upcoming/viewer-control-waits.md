---
title: "Viewer-control operations answer once they are done"
hidden: true
type: misc
---

### Viewer-control operations answer once they are done

`close_recordings`, `set_time_cursor`, `set_blueprint`, and `run_command` now answer once the Viewer has carried them out, so the next `viewer_state()` sees the result.
See the [MCP reference](../reference/viewer/mcp.md).
