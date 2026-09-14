---
title: "`--connect` also starts a local Viewer server"
hidden: true
type: breaking
---

Starting the Viewer with `--connect` now also starts a local Viewer server containing a message proxy, the viewer-control service, and the Viewer catalog.
The Viewer server uses a free port by default; pass `--port` to select one explicitly.
Rerun warns if `--port` matches the upstream message proxy port.
Bare `--connect` targets the upstream message proxy at `rerun+http://127.0.0.1:9876/proxy`; use `--connect 4321` to select another port.
Previously, `--connect` did not start a Viewer server, and `--connect --port 4321` selected the upstream message proxy port.
