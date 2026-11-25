---
title: Catalog write tools in the MCP server
hidden: true
type: feature
---

### Catalog write tools in the MCP server

The catalog tools of `rerun mcp` and of the Viewer's agent panel can now create datasets, rename datasets and tables, upload or register `.rrd` recordings into datasets, and register `.rbl` table blueprints as a dataset's default.
Each change asks you first, and none deletes or replaces data.
To keep an agent read-only, set "Catalog servers" to "Read only" in the agent panel's settings, or start `rerun mcp --catalog-read-only`.
See the [MCP server documentation](../viewer/extend/mcp.md#changing-a-catalog).
