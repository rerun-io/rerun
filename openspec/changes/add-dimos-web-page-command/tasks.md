## 1. Protocol and parsing

- [x] 1.1 Add a failing test or focused assertion for parsing an inbound `open_web_page_view` websocket command.
- [x] 1.2 Add a `WsCommand::OpenWebPageView` protocol type with `panel_id`, `title`, `url`, and `show_navigation_controls` fields.
- [x] 1.3 Keep existing outbound `WsEvent` messages (`click`, `twist`, `stop`) wire-compatible.

## 2. Bidirectional websocket plumbing

- [x] 2.1 Add a non-blocking incoming command queue from the websocket read loop to the DimOS viewer wrapper.
- [x] 2.2 Ensure ping/pong and reconnect behavior still works while inbound command frames are parsed.
- [x] 2.3 Log and ignore malformed or unknown inbound websocket messages without crashing the viewer.

## 3. Blueprint command application

- [x] 3.1 Inspect and choose the least invasive path for applying Web Page View blueprint updates from `DimosApp`.
- [x] 3.2 Add a helper that creates a Web Page View blueprint entry and saves `WebPageViewConfig` for a valid command.
- [x] 3.3 Add runtime-only `panel_id -> ViewId` tracking in `DimosApp`.
- [x] 3.4 Implement idempotent command handling: create for new `panel_id`, update existing panel for repeated `panel_id`.
- [x] 3.5 Focus the created or updated panel through existing viewport focus behavior when feasible.

## 4. Validation and scope control

- [x] 4.1 Validate command URLs with the same `http`/`https` policy used by Web Page View.
- [x] 4.2 Verify unsupported schemes do not create or update panels.
- [x] 4.3 Verify v1 ignores or rejects layout-placement fields rather than treating them as viewport-tree commands.
- [x] 4.4 Keep implementation localized to `dimos/`; pause for design review if core viewer APIs must be changed.

## 5. PR formalization

- [x] 5.1 Update PR notes or documentation text with the canonical Python/blueprint API example.
- [x] 5.2 Explicitly label the DimOS websocket command as experimental, DimOS-only, and removable.
- [x] 5.3 Document final file-scope justification: generated Web Page View blast radius belongs to the core feature; DimOS command changes stay separate and small.

## 6. Checks

- [x] 6.1 Run focused Rust tests for DimOS websocket command parsing/handling.
- [x] 6.2 Run relevant formatting/check commands for touched Rust files.
- [x] 6.3 Run `openspec validate "add-dimos-web-page-command" --strict --json`.
