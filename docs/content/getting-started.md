---
title: Getting Started
order: 1
description: Install Rerun, then log, visualize, query, and train on robotics data
---

Rerun helps robotics and Physical AI teams iterate faster: log from any sensor, visualize in the Viewer, query with dataframes, and train with a dataloader tailored to robotic learning — across one recording or many. <!-- TODO(RR-6462): update this Rerun description to the new branding -->
This page installs Rerun, opens the Viewer, and walks through each of those steps.

## Installation

`pip install rerun-sdk[catalog,dataloader]` bundles the **SDK** (log/query from code) and the **Viewer** (visualizer app). The optional dependencies support queries and training below.
For Rust, C++, see [Install Rerun](./getting-started/install-rerun.md) and [Set up a project](./getting-started/project-setup.md).

## Open the Viewer

`rerun` launches the Viewer.
Pass a file to open it directly:

```bash
rerun path/to/recording.rrd
```

Supports `.rrd`, `.mcap`, and [more](./ingest/open-any-file.md).
Also available in-browser at [rerun.io/viewer](https://rerun.io/viewer).


## Scale across many recordings

Rerun's catalog organizes recordings as queryable [**segments**](./catalog/catalog-object-model.md).
To query across recordings, log (or convert) data to an `.rrd`, start a local catalog server (or connect to [Rerun Hub](./catalog/hub.md), our commercial managed offering for production deployments), register the `.rrd` as a segment, then visualize and query across recordings. The Viewer can also open an `.rrd` in its built-in catalog without a separate server. <!-- TODO(RR-6462): update this Rerun Hub description to the new branding -->

### Log

Save data to an `.rrd`, see the [logging tutorial](./log/tutorial.md) for more details.
If you already have data in another format see [Ingest](./ingest.md) for various examples converting to `.rrd`.

snippet: tutorials/getting_started_log
### Start a catalog server

Start a local catalog on port `51234` with `rerun server`, then connect from your code.
If you use [Rerun Hub](./catalog/hub.md), connect to it instead (see [Run the examples on Hub](./catalog/hub.md#run-the-examples-on-hub)).

```bash
rerun server
```

snippet: tutorials/getting_started[setup]

### Ingest

Register an `.rrd` with a dataset so it shows up as a queryable segment.

snippet: tutorials/getting_started[ingest]

### Visualize

Point the Viewer at your server to browse every recording in the catalog.
See the [Viewer overview](viewer/organization.md).

```bash
rerun rerun+http://127.0.0.1:51234
```

### Query

Query the catalog into a [DataFusion](https://datafusion.apache.org/) DataFrame.
See the [query tutorial](./query-and-transform/tutorial.md).

snippet: tutorials/getting_started[query]

### Train

Connect a Dataloader to the server to generate training batches.
See the [training tutorial](./train/tutorial.md).

snippet: tutorials/getting_started[train]

## Work with an agent

Coding agents such as Claude Code and Codex can log, convert, and inspect your data for you.
See [Agents](./agents.md) for the Rerun skills, the MCP server, and the Viewer's agent panel.

## If you're stuck

-   Check the [troubleshooting guide](./getting-started/install-rerun/troubleshooting.md).
-   [Open an issue](https://github.com/rerun-io/rerun/issues/new/choose).
-   [Join the Discord server](https://discord.gg/Gcm8BbTaAj).
