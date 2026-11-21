---
title: Shared list of catalog servers
hidden: true
type: feature
---

### Shared list of catalog servers

The catalog servers you add in the Viewer are now also known to the CLI and to `rerun mcp`.
`rerun servers list`, `rerun servers add <url>`, and `rerun servers remove <url>` manage the list, and the `catalog_list_servers` MCP tool offers it to agents.
The native Viewer now only remembers servers you add or log in to, not those it merely opened from a URL.
On first start it moves the servers it remembered into the list, leaving out local ones.
The list lives in `servers.json` in Rerun's config directory, so Reset Viewer and `rerun reset` keep it, and a Viewer older than this release no longer shows these servers.
See the [MCP server documentation](../viewer/extend/mcp.md#reading-a-catalog).
