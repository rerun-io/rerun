---
title: Catalog tools in the MCP server
hidden: true
type: feature
---

### Catalog tools in the MCP server

`rerun mcp` now has `catalog_*` tools that connect to a Rerun catalog server directly. They can list and query datasets, their segments, schemas, segment properties, and data, with SQL over tables and query results.
By default they act on the catalog of the connected Viewer (if any); pass `--catalog-url <url>` for another server, and `--toolsets catalog` to run them without a Viewer.
See the [MCP server documentation](../viewer/extend/mcp.md#reading-a-catalog).
