---
title: "The web viewer's analytics work like the native viewer's"
hidden: true
type: breaking
---

The `@rerun-io/web-viewer` npm package now sends usage data, like the native viewer and rerun.io/viewer already do.
The first time a browser opens a web viewer that sends usage data, the viewer shows a one-time notice.
Like the native viewer, it sends nothing during that first visit, so users can turn usage data off before anything is sent.
Users can turn usage data off in Settings, in every viewer.
The new `analytics` option of `WebViewerOptions` is `"on"` (the default) or `"off"`.
Pages that embed the viewer in an iframe choose the same way with the `analytics` query parameter, e.g. `?url=…&analytics=off`.
Web viewers started by `rerun --web-viewer`, `rr.serve_web_viewer()` and the notebook follow `rerun analytics disable`.

#### Migration

To keep the old behavior of the npm package and send no usage data, set `analytics` to `"off"`:

```ts
const viewer = new WebViewer();
await viewer.start(url, parent, { analytics: "off" });
```

If you leave usage data on in an application you ship, consider mentioning Rerun in your privacy policy.
