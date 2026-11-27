---
title: MCP server
order: 300
description: Connect an agent to the Rerun Viewer to inspect recordings, change layouts, or query a catalog
---

The Rerun MCP server lets an agent, such as Claude Code or Codex, inspect recordings in a running Viewer, change its layout, and query a catalog.
This page shows how to install it and what the agent can do with it.

## Install the MCP server

The Model Context Protocol (MCP) server comes with the Rerun CLI. If you need the CLI, install it with Python or choose [another installation method](../../getting-started/install-rerun/viewer.md).

```sh
pip install rerun-sdk
```

To register the server with **Claude Code**, run the following command.

```sh
claude mcp add rerun -- rerun mcp
```

For **Codex**, use this command.

```sh
codex mcp add rerun -- rerun mcp
```

For other MCP clients, set the command to `rerun` and the argument to `mcp` (stdio). Open a recording in the Viewer, or ask the agent to start one with `rerun --headless` to work without a display.

## What the MCP server can access

The MCP server gives your agent access to two things:

- A data catalog on [Rerun Hub](../../catalog/hub.md) or on an [open-source server](../../catalog/oss-server.md).
- An open Rerun Viewer, which the agent can inspect and control.

To choose a catalog or Viewer at startup, see the [CLI options](../../reference/cli.md#rerun-mcp). You can also use the [agent panel](../../agents.md#agent-panel) inside the Viewer.

<a id="reading-a-catalog"></a>

### Catalog tools

Use the catalog tools to find and query data across recordings. You can connect directly to a catalog without opening the Viewer. If you connect to a local Viewer, its catalog is the default.

Ask “Which recordings contain lidar data?” to explore datasets and segment properties, or “What was the peak speed in each run?” to query recorded values with SQL.

The agent can also list the catalog servers you added in the Viewer or with `rerun servers add <url>`, and pick one to connect to.
The web Viewer cannot offer that list, since it keeps its servers in the browser rather than in Rerun's config directory.

<a id="changing-a-catalog"></a>

The agent can also create and rename datasets, upload or register `.rrd` recordings into them, and register a table blueprint for a dataset's segment table, optionally as its default.
Ask “Upload `/data/run_42.rrd` into a new dataset called `runs`”.
These tools never delete or replace data, and the agent asks you before every change, since changes are shared with everyone using the catalog.
To keep the agent read-only, start the server with `rerun mcp --catalog-read-only`, or set "Catalog servers" to "Read only" in the agent panel's settings.

<a id="what-the-agent-sees"></a>

### Viewer tools

Your agent can use the viewer tools to set up your viewer, or answer questions about the data you are working with.

<a id="reading-and-writing-the-blueprint"></a>

A [blueprint](../inspect-recordings/recording-layout.md) describes how the Viewer displays your data. Ask “Put the two cameras side by side, with the 3D view below them”. The agent can apply the layout to your open recording, and you can undo the edit in the Viewer.

<a id="inspecting-transforms"></a>

The agent can trace how coordinate frames connect at the current time. Try “Show me how the left camera connects to the robot base”.

<a id="component-defaults-for-a-whole-view"></a>

To set a default style across a view, ask the agent to change its [component defaults](../view-concepts/customize-views.md#set-a-default-for-a-view).

<a id="component-overrides"></a>

To highlight one entity, ask “Make the lidar points red in the 3D view”. The override affects only that view and takes precedence over the recorded color. See [Customize views](../view-concepts/customize-views.md#set-a-value-for-one-entity).

<a id="table-blueprints"></a>

An agent can filter rows, rearrange columns, or show recordings as cards with camera previews. See [Configure table layouts and segment previews](../browse-datasets/table-layout.md).

<a id="changing-settings"></a>

The agent can read and change the Viewer's app settings, such as the memory limit, the timestamp format, and the video decoder options.
Try “Raise the memory limit to 8 GiB”.
The Viewer saves the change, so it outlives the session.
The agent cannot read or change secrets, such as the Mapbox access token.
