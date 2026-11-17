---
title: "`rerun viewer-mcp` is now `rerun mcp`"
hidden: true
type: breaking
---

### `rerun viewer-mcp` is now `rerun mcp`

The MCP server's subcommand is now `rerun mcp`, since it will offer more than Viewer control.
Re-register it with your agent:

```sh
# Before
claude mcp add rerun -- rerun viewer-mcp

# After
claude mcp add rerun -- rerun mcp
```

In an `mcp.json`, change `"args": ["viewer-mcp"]` to `"args": ["mcp"]`.
If you added it under a name other than `rerun` in the Viewer's agent panel settings, change its command line there too.
The standalone binary for Rerun developers is now `rerun-mcp`, built from the `re_rerun_mcp` crate.
