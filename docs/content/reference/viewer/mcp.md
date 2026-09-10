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

By default the agent picks the Viewer with the `connect` tool.
To bind the server to one Viewer up front, pass its gRPC address: `rerun viewer-mcp --endpoint http://127.0.0.1:9876`.
The server then connects on startup, so the agent can skip `connect`.
If the Viewer is not running yet, the server tells the agent to call `connect` instead.

To check whether an address is a Rerun Viewer before connecting, use [gRPC server reflection](../grpc.md): `grpcurl -plaintext 127.0.0.1:9876 list` names the services it speaks.

## Headless usage

The MCP server works against a headless Viewer too, which is convenient for agents running in the background, in CI, or on a server without a display.
Ask the agent to launch the Viewer headless or in the background, and it will start it with `rerun --headless`.

## What the agent sees

Besides the accessibility tree and screenshots, the server gives the agent the same signals a user relies on to tell whether the Viewer is healthy:

- **View reports**: `viewer_state` lists every view of the current blueprint together with the warnings and errors it reported the last time it was shown, the same ones behind the warning icon in a view's title bar.
  That tells the agent why a view is empty or looks wrong without a screenshot.
- **Viewer logs**: the Viewer's log messages (INFO and above) since the previous tool call are appended to every tool result, and the `viewer_logs` tool returns the recent history.
  The agent notices the same warnings and errors you see in the notification panel.

## Reading the data

The MCP tools drive the UI; they deliberately do not read data.
Instead, the Viewer hosts a catalog server, and `viewer_state` reports its address as `catalog_url`, along with each recording's `application_id` and `recording_id`, which are its dataset id and segment id in that catalog.
The server instructions tell the agent to query that server with the Python `rerun.catalog` API for entity paths, components, and values rather than guessing them.
For a recording that is not in the catalog, such as one streamed from an SDK or imported from another file format, the agent is told to read the file directly with `rerun.chunk.RrdReader` instead.

## Closing recordings

The `close_recordings` tool removes one, many, or all recordings from the Viewer.
An agent that regenerates a file and reopens it repeatedly otherwise accumulates stale recordings, which makes both `viewer_state` and the recording panel hard to read.
Closing a recording does not touch files on disk, but unsaved blueprint edits to it are lost.
A registered recording also stays in the catalog, so it can still be read over the catalog API and reopened later.
