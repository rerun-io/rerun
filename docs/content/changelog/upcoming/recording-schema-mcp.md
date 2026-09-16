---
title: "Agents can read a recording's schema over MCP"
hidden: true
type: feature
---

### Agents can read a recording's schema over MCP

`GetViewerState` — and with it the `rerun_get_viewer_state` MCP tool and `ViewerClient.viewer_state()` — now reports the Viewer's version, so a caller no longer has to shell out to `rerun --version` and hope it found the same binary.

The new `GetRecordingSchema` operation, served as the `rerun_get_recording_schema` MCP tool, describes an open recording: every entity, the components logged on each, whether a component has a static value, and its Arrow datatype.

```json
{"component": "Points3D:positions",
 "archetype": "rerun.archetypes.Points3D",
 "component_type": "rerun.components.Position3D",
 "datatype": "FixedSizeList(3 x non-null Float32)",
 "has_static": true}
```

It works for any recording the Viewer has open, including one streamed from an SDK or imported from another file format, which the catalog cannot answer for.
A recording too large for one answer comes back truncated with a count of what was left out, and `entity_path` narrows to a subtree.
`paths_only` answers the cheaper question first — which entities exist at all — so an unfamiliar recording can be listed whole and then read in detail only where it matters.

See the [MCP reference](../reference/viewer/mcp.md).
