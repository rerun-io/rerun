---
title: "Agents can read and write the active blueprint over MCP"
hidden: true
type: feature
---

### Agents can read and write the active blueprint over MCP

The new `GetBlueprint` operation, served as the `rerun_get_blueprint` MCP tool, returns a recording's active blueprint as JSON, one entry per blueprint entity with its archetypes and their fields:

```json
{
  "/view/1f2e…": { "ViewBlueprint": { "class_identifier": "3D", "space_origin": "/world" } },
  "/view/1f2e…/ViewContents": { "ViewContents": { "query": "+ /world/**" } },
  "/viewport": { "ViewportBlueprint": { "root_container": "4a8c…" } }
}
```

The new `SetBlueprint` operation, served as `rerun_set_blueprint`, replaces the whole blueprint with JSON of the same form.

See the [MCP reference](../reference/viewer/mcp.md#reading-and-writing-the-blueprint).
