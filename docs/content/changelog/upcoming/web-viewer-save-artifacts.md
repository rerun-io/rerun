---
title: Export recordings and blueprints from the Web Viewer
hidden: true
type: feature
---

### Export recordings and blueprints from the web Viewer

The [Web Viewer JavaScript API](../../reference/npm.md) now provides `save_recording()` and `save_blueprint()`.
Each method returns a byte stream that can be saved as an `.rrd` or `.rbl` file.
The stream is also compatible with `open_channel()`, so applications can restore an exported artifact through normal RRD ingestion.

```ts
const artifact = new Uint8Array(
  await new Response(viewer.save_recording()).arrayBuffer(),
);
// …
const channel = viewer.open_channel();
channel.send_rrd(artifact);
```
