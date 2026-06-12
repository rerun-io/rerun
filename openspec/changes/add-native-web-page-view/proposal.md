## Why

Rerun users can compose rich native viewer layouts for logged data, but cannot place a live webpage alongside 3D, image, and other views. A native inline Web Page View enables dashboards, local robot control panels, documentation, and other http(s) pages to appear inside the Rerun viewer layout instead of requiring a separate browser window.

## What Changes

- Add a native-only **Web Page View** that displays a configured webpage inline in the viewer layout.
- Store the view's URL and lightweight browser chrome preference as blueprint/view configuration, not logged timeline data.
- Allow manual creation through the viewer UI and preconfiguration through blueprint state.
- Autoload configured `http://` and `https://` URLs, including localhost and private-network URLs.
- Reject unsupported URL schemes such as `file:`, `data:`, `javascript:`, and custom schemes with clear Rerun-side status UI.
- Use direct native webview integration for wry-supported native platforms.
- Keep runtime navigation browser-like while leaving the configured URL unchanged.
- Share embedded-browser session state by default; per-view isolated profiles remain out of scope for the initial version.
- Do not support this view in the web viewer.

## Capabilities

### New Capabilities
- `native-web-page-view`: Defines native Web Page View behavior, configuration, platform support, URL policy, lifecycle, navigation, session handling, and error reporting.

### Modified Capabilities

None.

## Impact

- Viewer view registration and manual view creation UI.
- Blueprint view definitions and generated SDK/view property types.
- Native viewer platform integration and dependency graph for embedded webviews.
- Native-only runtime lifecycle for one webview instance per Web Page View.
- Documentation and tests for blueprint configuration, URL validation, unsupported targets, and native webview lifecycle behavior.
