## 1. TDD Tracer Bullet: Blueprint View Skeleton

- [x] 1.1 RED: Add one behavior test that expects a manually created Web Page View with no URL to render a Rerun-side "No URL configured" status through the public view/test harness.
- [x] 1.2 GREEN: Add the minimal `WebPageView` blueprint definition, generated code, `re_view_web_page` crate/module skeleton, view registration, and empty-URL status UI needed to pass 1.1.
- [x] 1.3 REFACTOR: Align names with existing view conventions (`Web Page View` user-facing name, `web_page`/`WebPageView` code names as appropriate) and run `pixi run rs-fmt` plus the smallest relevant Rust check.

## 2. TDD Slice: Blueprint Configuration and Manual Creation

- [x] 2.1 RED: Add a behavior test that creates a Web Page View from blueprint state containing `url` and `show_navigation_controls`, and verifies those values are read by the view without logged data.
- [x] 2.2 GREEN: Implement blueprint properties for required URL and defaulted `show_navigation_controls = true`; ensure manual creation works and data-driven spawn heuristics do not suggest the view.
- [x] 2.3 REFACTOR: Keep configuration access isolated behind a small typed helper so later backend code does not parse blueprint state directly.

## 3. TDD Slice: URL Policy and Status Errors

- [x] 3.1 RED: Add behavior tests for accepted `https://example.com`, accepted `http://localhost:3000`, rejected `file:///tmp/report.html`, and invalid URL text.
- [x] 3.2 GREEN: Implement URL parsing/validation that allows only `http` and `https`, including localhost/private-network HTTP, and renders Rerun-side status messages for invalid or unsupported URLs.
- [x] 3.3 REFACTOR: Move URL policy into a backend-independent unit with focused tests so native webview code can trust validated URLs.

## 4. TDD Slice: Native Webview Backend Seam

- [x] 4.1 RED: Add tests using a fake backend that verify a valid configured URL causes exactly one backend webview instance to be created for one Web Page View.
- [x] 4.2 GREEN: Introduce a narrow native webview backend abstraction and wire the view to it with a fake/test backend; do not depend on `wry` in behavior tests.
- [x] 4.3 RED: Add a behavior test that two Web Page Views with different URLs create two independent backend instances.
- [x] 4.4 GREEN: Implement per-view instance ownership keyed by stable view identity.
- [x] 4.5 REFACTOR: Keep egui/view logic, lifecycle manager, and platform backend responsibilities separate.

## 5. TDD Slice: Direct wry Integration

- [x] 5.1 RED: Add a compile-gated native integration test or smoke test target that exercises construction through the real native backend boundary and reports backend creation failure instead of panicking.
- [x] 5.2 GREEN: Add direct `wry` integration for native builds, including required Cargo feature/dependency wiring and platform-specific creation paths supported by wry.
- [x] 5.3 REFACTOR: Encapsulate platform-specific wry details behind the backend boundary, including Linux WebKitGTK/X11/Wayland caveats and bounds-setting differences.

## 6. TDD Slice: Layout Bounds and Lifecycle

- [x] 6.1 RED: Add fake-backend behavior tests that verify the backend receives updated bounds when the egui view rectangle changes.
- [x] 6.2 GREEN: Update native webview bounds from the allocated egui view rectangle each frame, accounting for DPI/points-to-pixels conversion.
- [x] 6.3 RED: Add fake-backend behavior tests that verify hiding a view keeps its instance alive and removing a view destroys its instance.
- [x] 6.4 GREEN: Implement keep-alive-while-hidden lifecycle and destroy-on-remove/app-exit cleanup.
- [x] 6.5 REFACTOR: Audit focus, clipping, tab, split-panel, and resize behavior with the fake backend before manual native smoke testing.

## 7. TDD Slice: Navigation Controls and Runtime Navigation

- [x] 7.1 RED: Add behavior tests that navigation controls are visible by default and hidden when `show_navigation_controls` is false.
- [x] 7.2 GREEN: Implement lightweight back, forward, reload, home, and URL display controls above the embedded webview, reserving the full view area when controls are hidden.
- [x] 7.3 RED: Add a behavior test that runtime navigation does not mutate the blueprint-configured URL and that Home navigates back to the configured URL.
- [x] 7.4 GREEN: Wire navigation commands through the backend abstraction while keeping configured URL state immutable during runtime navigation.

## 8. TDD Slice: Platform Support, Session Defaults, and Failure UI

- [x] 8.1 RED: Add behavior tests that web builds or unavailable native backends render explicit unsupported/failure status UI.
- [x] 8.2 GREEN: Implement unsupported-target and backend-failure reporting outside the webview surface.
- [x] 8.3 RED: Add a fake-backend behavior test that multiple views use the shared default browser profile/session configuration.
- [x] 8.4 GREEN: Implement shared default session/profile behavior in the backend boundary while leaving per-view isolation as a future extension point.

## 9. Verification and Documentation

- [x] 9.1 Run `pixi run codegen` after blueprint definition changes and verify generated Rust/Python/C++ outputs are updated as expected.
- [x] 9.2 Run `pixi run rs-fmt` after Rust changes.
- [x] 9.3 Run targeted Rust checks/tests for the new view crate and affected viewer crates with `cargo clippy -p <crate_name>` and `cargo nextest run --all-features --no-fail-fast -p <crate_name>` (`cargo nextest` was unavailable in this environment; used focused `cargo test` plus clippy coverage instead).
- [x] 9.4 Manually smoke-test native Web Page View creation, URL editing, split/tab resizing, navigation controls, multiple simultaneous views, hidden-tab preservation, and backend failure messaging on at least one supported native platform.
- [x] 9.5 Document native platform/runtime requirements, including WebView2/WebKitGTK expectations and the fact that the Web Page View is unsupported in the web viewer.
