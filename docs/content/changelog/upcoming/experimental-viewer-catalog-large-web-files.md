---
title: "Experimental Viewer catalog: larger-than-RAM files on the web"
hidden: true
type: highlight
---

### Experimental Viewer catalog: larger-than-RAM files on the web

The [experimental Viewer catalog](changeset-0-35.md#experimental-viewer-catalog) can now load `.rrd` files lazily in the web Viewer.
Files opened through the file dialog or drag-and-drop are streamed into the browser's Origin Private File System without passing through Wasm linear memory, and recording chunks are then read on demand.
This lets the web Viewer open recordings larger than available RAM and even larger than Wasm's 4 GiB address space.
Reopening the same file reuses its content-addressed browser-storage copy, when persistence is enabled.

Embedded default blueprints are now preserved when an `.rrd` is registered, so recordings open with their intended layout.
The Viewer catalog also stays hidden in the recording panel until it contains data.

To try it, open **Settings**, then enable **Load files via Viewer catalog** under **Experimental** before opening the `.rrd` file.
For large files on the web, also select **Request persistence** under **Origin private filesystem**.
This asks the browser to protect Viewer catalog files from automatic storage eviction and may increase the storage quota in some browsers; the browser can still deny the request.
