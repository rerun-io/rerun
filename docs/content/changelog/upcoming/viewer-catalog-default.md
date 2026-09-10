---
title: "Local `.rrd` files load via the Viewer catalog by default"
hidden: true
type: highlight
---

### Local `.rrd` files load via the Viewer catalog by default

The Viewer catalog now loads local `.rrd` files by default, making the complete recording navigable almost instantly while chunks load on demand.
This makes it possible to directly load recordings that are larger-than-RAM.
Using the same feature, the web viewer can now also load files that are larger then the Wasm-addressable memory.

Note, currently the Viewer catalog comes with some (minor) limitation around blueprints:

* Embedded default blueprints still work, but only the last `send_blueprint(..., make_default=True)` is loaded into the catalog.
* `make_active`-only blueprints and blueprints sent after the file opens are not applied to its catalog-backed recording.

To restore the previous behavior, you can opt out under **Settings** → **Viewer catalog** → **Load files via Viewer catalog**.
If you choose to do so, we'd love to hear your feedback on how to better accommodate your workflows!

<picture>
  <img src="https://static.rerun.io/viewer-catalog-settings/49cb028512b37bffa818b125c0a984b3878f611d/full.png" alt="Settings entry for toggling Viewer catalog">
</picture>
