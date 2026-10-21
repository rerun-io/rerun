---
title: Inspect transform graphs via MCP
hidden: true
type: highlight
---

Rerun already provides a [transform debugging UI](../concepts/logging-and-ingestion/transforms.md#debugging-transform-trees) for inspecting frame connections and transform values at the current time cursor.
Agents can now inspect that same transform graph via MCP using `rerun_get_transform_graph`, which returns frame connections and their sources as JSON, with optional filters and numerical transform values to help debug spatial relationships.
See [Inspecting transforms via MCP](../reference/viewer/mcp.md#inspecting-transforms) for details.
