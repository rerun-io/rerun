# Rerun web viewer

Embed the Rerun web viewer within your app.

<p align="center">
  <picture>
    <img src="https://static.rerun.io/opf_screenshot/bee51040cba93c0bae62ef6c57fa703704012a41/full.png" alt="">
    <source media="(max-width: 480px)" srcset="https://static.rerun.io/opf_screenshot/bee51040cba93c0bae62ef6c57fa703704012a41/480w.png">
    <source media="(max-width: 768px)" srcset="https://static.rerun.io/opf_screenshot/bee51040cba93c0bae62ef6c57fa703704012a41/768w.png">
    <source media="(max-width: 1024px)" srcset="https://static.rerun.io/opf_screenshot/bee51040cba93c0bae62ef6c57fa703704012a41/1024w.png">
    <source media="(max-width: 1200px)" srcset="https://static.rerun.io/opf_screenshot/bee51040cba93c0bae62ef6c57fa703704012a41/1200w.png">
  </picture>
</p>

This package is framework-agnostic. A React wrapper is available at <https://www.npmjs.com/package/@rerun-io/web-viewer-react>.

## Install

```
$ npm i @rerun-io/web-viewer
```

ℹ️ Note:
The package version is equal to the supported Rerun SDK version, and [RRD files are not yet stable across different versions](https://github.com/rerun-io/rerun/issues/6410).
This means that `@rerun-io/web-viewer@0.10.0` can only connect to a data source (`.rrd` file, websocket connection, etc.) that originates from a Rerun SDK with version `0.10.0`!

## Usage

The web viewer is an object which manages a canvas element:

```js
import { WebViewer } from "@rerun-io/web-viewer";

const rrd = "…";
const parentElement = document.body;

const viewer = new WebViewer();
await viewer.start(rrd, parentElement, { width: "800px", height: "600px" });
// …
viewer.stop();
```

The `rrd` in the snippet above should be a URL pointing to either:
- A hosted `.rrd` file, such as <https://app.rerun.io/version/0.21.0/examples/dna.rrd>
- A WebSocket connection to the SDK opened via the [`serve`](https://www.rerun.io/docs/reference/sdk/operating-modes#serve) API

If `rrd` is not set, the Viewer will display the same welcome screen as <https://app.rerun.io>.
This can be disabled by setting `hide_welcome_screen` to `true` in the options object of `viewer.start`.

⚠ It's important to set the viewer's width and height, as without it the viewer may not display correctly.
Setting the values to empty strings is valid, as long as you style the canvas through other means.

For a full example, see https://github.com/rerun-io/web-viewer-example.
You can open the example via CodeSandbox: https://codesandbox.io/s/github/rerun-io/web-viewer-example

ℹ️ Note:
This package only targets recent versions of browsers.
If your target browser does not support Wasm imports or top-level await, you may need to install additional plugins for your bundler.
