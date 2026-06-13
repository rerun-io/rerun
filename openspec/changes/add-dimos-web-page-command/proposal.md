## Why

DimOS needs a pragmatic way to request a Web Page View panel from its existing websocket control path while preserving Rerun's canonical blueprint/Python API as the source of truth for viewer layout. This lets reviewers evaluate the DimOS convenience command independently from the core native Web Page View implementation.

## What Changes

- Add a DimOS-only websocket command, `open_web_page_view`, that requests a Web Page View panel by caller-owned `panel_id`.
- Translate the websocket command into Web Page View blueprint state instead of directly mutating native webview internals.
- Keep a runtime-only `panel_id -> ViewId` mapping in the DimOS viewer wrapper so repeated commands update/focus the same panel.
- Keep layout placement intentionally minimal for v1: create/update/focus the logical panel, but do not expose split direction, size ratios, tab placement, or arbitrary viewport-tree control.
- Preserve the canonical API story: updated Rerun SDK users should use `rr.send_blueprint(rrb.WebPageView(...))`; the DimOS websocket command is an experimental/removable convenience path.

## Capabilities

### New Capabilities

- `dimos-web-page-command`: DimOS websocket command for opening/updating/focusing native Web Page View panels.

### Modified Capabilities

- `native-web-page-view`: Clarifies that Web Page View remains blueprint-owned and can be requested by a DimOS websocket convenience wrapper without changing the canonical Rerun blueprint API.

## Impact

- Affected DimOS code: websocket protocol handling in `dimos/src/interaction/ws.rs` and viewer wrapper command handling in `dimos/src/viewer.rs`.
- Possible small helper/export changes under `dimos/src/interaction/` if needed to keep parsing and idempotency logic isolated.
- Should not require new generated Rerun SDK/schema/codegen changes, native webview backend changes, or broader viewer core changes unless blueprint application from the DimOS wrapper proves blocked.
- PR narrative should explicitly separate the existing core Web Page View implementation from this DimOS-only convenience command.
