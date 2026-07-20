---
title: MCP server
order: 5
---

The Rerun CLI includes an [MCP](https://modelcontextprotocol.io/) server that lets agents such as Codex or Claude interact with a running Viewer.
It allows the agent to interact with the viewer like a real user, allowing it to interact with the ui, adjust settings, type text, or take screenshots.
It works similar to e.g. Claude for Chrome or Codex Computer Use, but tailored to Rerun.

Some things it is useful for:

- **Debugging a logging script**: "The left camera doesn't show up in the viewer, investigate and fix via the mcp."
- **Adding a custom blueprint**: "Create a blueprint with two tabs: The first is a grid of the cameras, the second shows the map and 3D view. Verify with rerun viewer-mcp."
- **Explore recordings**: "Look at each recording in this dataset and find where it rains. Write a report including screenshots."

## Setup

The server is the `viewer-mcp` subcommand of the `rerun` binary, speaking MCP over stdio.
It connects to a separate, already-running Viewer over gRPC, so an MCP client only needs to know how to launch `rerun viewer-mcp`.

Add it to **Claude Code**:

```sh
claude mcp add rerun -- rerun viewer-mcp
```

Add it to **Codex**:

```sh
codex mcp add rerun -- rerun viewer-mcp
```

Or configure any MCP client manually. Most accept a `mcp.json` config like this:

```json
{
  "mcpServers": {
    "rerun": {
      "command": "rerun",
      "args": ["viewer-mcp"],
      "env": {
        "RUST_LOG": "re_viewer_mcp=info,warn"
      }
    }
  }
}
```

These assume `rerun` is installed on your `PATH` (see [install rerun](../../getting-started/install-rerun.md)).
If it is not, replace `rerun` with the absolute path to the binary.

## Headless usage

The MCP server works against a headless Viewer too, which is convenient for agents running in the background, in CI or
on some server without a display.
Ask the agent to launch the viewer headless or in the background, and it will use the `rerun --headless` command to
launch it in the background.
