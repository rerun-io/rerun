## Context

Rerun's native viewer supports composable view types such as spatial, image, map, text, tensor, and dataframe views. Built-in views are registered through the viewer's view class registry, and view-specific configuration is represented as blueprint state generated from blueprint type definitions.

There is currently URL-opening plumbing for loading Rerun data sources and opening browser URLs, but no view type that owns an embedded browser surface. The closest existing pattern is the map view: it is a registered view class with generated blueprint properties and native UI behavior.

The Web Page View introduces a native-only view that displays a configured `http(s)` page inline in the viewer layout. It is not a visualizer for logged entities and has no timeline semantics.

## Goals / Non-Goals

**Goals:**

- Add a first-class Web Page View that can be manually added to a layout and preconfigured through blueprint state.
- Store the initial/home URL and navigation chrome preference as view configuration.
- Display one live embedded native webview per Web Page View on wry-supported native platforms.
- Validate URL schemes before creating/loading the webview.
- Provide clear Rerun-side status UI for missing configuration, invalid URLs, unsupported targets, and backend creation failures.
- Shape implementation tasks as TDD vertical slices: one observable behavior test, minimal implementation, repeat.

**Non-Goals:**

- Supporting Web Page View in the web viewer.
- Driving the displayed URL from logged timeline data.
- Auto-spawning Web Page Views from entity contents.
- Supporting `file:`, `data:`, `javascript:`, or custom URL schemes.
- Implementing per-view isolated browser profiles in the initial version.
- Adding advanced browser options such as custom user agent, devtools configuration, zoom factor, transparent background, or script injection.

## Decisions

### Native-only Web Page View

The Web Page View is available only in native viewer builds. The web viewer should render a clear unsupported status instead of attempting iframe support.

**Alternatives considered:**
- Support the web viewer with iframes. Rejected because the web viewer already runs inside a browser and iframe behavior depends on cross-origin policy, CSP, and `X-Frame-Options`.
- Link-only view. Rejected because the desired behavior is an inline embedded webpage, not a shortcut to an external browser.

### Blueprint/view configuration, not logged data

The view's `url` and `show_navigation_controls` fields are blueprint/view configuration. The view has no data visualizer and should not be suggested from logged entities.

**Alternatives considered:**
- Introduce a logged webpage archetype. Rejected for the initial version because webpage selection is layout configuration, not time-aware data in the requested use case.
- Support both logged and blueprint URLs. Deferred to avoid conflicting ownership of the current page.

### Direct wry integration

Use a native integration layer backed by `wry` rather than an egui wrapper crate. The integration should translate each view's egui rectangle into native webview bounds and manage webview lifecycle separately from egui painting.

**Alternatives considered:**
- Third-party egui webview wrappers. Rejected as the product direction because available wrappers appear experimental and would still depend on wry/platform behavior.
- Browserless rendering or screenshots. Rejected because the view must display live interactive webpages.

### One webview instance per view

Each Web Page View owns one native webview instance. Multiple views therefore render independently and can show multiple live pages at once.

**Alternatives considered:**
- Share one webview instance between views. Rejected because it conflicts with Rerun's composable layout model.
- Limit to one Web Page View per app. Rejected as an artificial limitation.

### Keep instances alive while views exist

The initial behavior keeps a webview alive while its Rerun view exists, including when the view is temporarily hidden. The instance is destroyed when the view is removed or the viewer exits.

**Alternatives considered:**
- Destroy on hide. Rejected because it would reload dashboards, lose scroll position, and disrupt login/application state.

### Browser-like navigation with stable configured URL

The configured `url` is the initial/home URL. Runtime navigation inside the page does not mutate blueprint state. If navigation controls are visible, home returns to the configured URL.

**Alternatives considered:**
- Lock navigation to the configured URL. Rejected because normal webpages rely on links and redirects.
- Persist every navigation into blueprint state. Rejected because casual browsing should not dirty saved layout configuration.

### Shared browser session by default

Web Page Views share the default embedded browser session/profile in the initial version. Per-view isolation should remain possible as a future extension.

**Alternatives considered:**
- Isolate each view by default. Rejected because common dashboard use cases would require repeated logins and duplicate session setup.

## Risks / Trade-offs

- Native webview surfaces may not clip, stack, or resize exactly like egui widgets → Keep the webview integration behind a narrow module boundary, update bounds from the egui view rectangle, and test split panels/tabs/resizing.
- Linux support depends on WebKitGTK/display-server details → Treat support as wry-supported native platforms, document dependencies, and render backend errors clearly.
- WebView2/WebKit runtime dependencies may be missing → Detect creation failures and show Rerun-side status UI instead of crashing.
- Shared session state is convenient but less isolated → Keep per-view profile configuration out of v1 while avoiding API choices that prevent it later.
- Embedded arbitrary webpages can consume significant CPU/memory → Keep instances alive for usability in v1; consider future suspend/destroy policies if resource usage becomes a problem.
- Native child views can complicate input focus and keyboard shortcuts → Ensure focus transfer between egui and webview is tested, especially navigation controls and viewer shortcuts.
