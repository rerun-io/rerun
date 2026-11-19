---
title: "MCP viewer tools are now named `viewer_*`"
hidden: true
type: breaking
---

### MCP viewer tools are now named `viewer_*`

The Viewer tools of `rerun mcp` are now prefixed `viewer_` instead of `rerun_`, e.g. `viewer_get_state` instead of `rerun_get_state`.
This keeps them apart from the catalog tools the server is gaining.
If your agent's settings allow or deny these tools by name, update those names:

```diff
- mcp__rerun__rerun_get_state
+ mcp__rerun__viewer_get_state
```
