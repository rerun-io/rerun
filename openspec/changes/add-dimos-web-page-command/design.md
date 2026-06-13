## Context

The native Web Page View implementation already exposes the canonical Rerun path for creating a web panel: users send blueprint state through the generated Rerun SDK, e.g. `rr.send_blueprint(rrb.WebPageView(config=rrb.WebPageViewConfig(url=...)))`. That path is official, typed, and transported through Rerun's existing log/gRPC channel.

DimOS also has an existing websocket control path in `dimos/src/interaction/ws.rs`. Today it is outbound from the viewer to the DimOS server for click, twist, and stop events; incoming frames are consumed only to keep the connection healthy. The DimOS viewer wrapper in `dimos/src/viewer.rs` wraps `re_viewer::App` for keyboard and selection behavior.

This change adds a small DimOS-only inbound command lane for requesting Web Page View panels while keeping Rerun blueprint state as the source of truth.

## Goals / Non-Goals

**Goals:**

- Add an experimental DimOS websocket command for opening/updating/focusing a Web Page View.
- Keep the command a thin convenience wrapper around Web Page View blueprint state.
- Keep `panel_id -> ViewId` state runtime-only inside the DimOS viewer wrapper.
- Keep the implementation localized to `dimos/` unless a narrowly-scoped core viewer API is required to apply blueprint updates cleanly.
- Make the DimOS command easy to remove if reviewers prefer the canonical Python/blueprint API only.

**Non-Goals:**

- Do not add new Rerun SDK schema/codegen for this DimOS command.
- Do not make the DimOS websocket a stable Rerun layout API.
- Do not expose arbitrary layout placement, split ratios, or viewport-tree editing in v1.
- Do not bypass the Web Page View URL policy or native backend lifecycle.
- Do not support old stock `rerun-sdk` Python packages as a typed Web Page View API; they can use the DimOS command as a pragmatic fallback.

## Decisions

1. **Canonical API remains Rerun blueprint/Python.**
   - Use `rr.send_blueprint(rrb.WebPageView(...))` as the official API story.
   - The DimOS websocket command is a convenience path, not a second source of truth.
   - Alternative considered: make DimOS websocket the primary panel API. Rejected because it duplicates Rerun's viewer-layout channel and would be harder for Rerun reviewers to accept.

2. **Inbound websocket command is caller-owned and idempotent by `panel_id`.**
   - Command shape includes `panel_id`, `title`, `url`, and `show_navigation_controls`.
   - Repeating the same `panel_id` updates/focuses the existing panel instead of creating duplicates.
   - Alternative considered: viewer-generated IDs. Rejected because it would require a response/lookup protocol before callers could update or close the panel.

3. **`panel_id -> ViewId` mapping is runtime-only.**
   - The DimOS wrapper stores the mapping in memory.
   - Restarting the viewer loses the mapping, which is acceptable for an experimental command.
   - Alternative considered: persist `panel_id` in blueprint metadata. Rejected because it would add schema/API surface solely for a DimOS convenience wrapper.

4. **Focus/raise on open/update if feasible.**
   - Existing `ViewportBlueprint::focus_tab(view_id)` supports focusing a view's tab through `ViewportCommand::FocusTab`.
   - `open_web_page_view` should make the requested panel visible to the user when possible.

5. **Keep layout placement out of scope.**
   - The command requests a logical panel, not a full remote layout edit.
   - If placement is needed later, it should be designed as a separate layout-control capability.

## Risks / Trade-offs

- **Risk: Applying blueprint changes from `DimosApp` may require a core viewer API.** → First inspect for an existing clean path. If unavailable, add the smallest focused viewer method/command needed rather than poking internal viewport state.
- **Risk: Two apparent APIs can confuse reviewers.** → PR text must explicitly label Python/blueprint as canonical and DimOS websocket as experimental/removable.
- **Risk: Runtime-only mapping loses identity across restart.** → Accept for v1; callers can resend commands after reconnect.
- **Risk: DimOS command bypasses typed Python validation.** → Reuse or mirror the same `http`/`https` URL policy before creating/updating the panel.
- **Risk: Scope creep into layout management.** → Keep v1 to create/update/focus only.
