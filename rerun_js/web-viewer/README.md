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

```sh
npm i @rerun-io/web-viewer
```

ℹ️ Note:
The package version is equal to the supported Rerun SDK version, and [RRD files are only partially stable across different versions](https://rerun.io/blog/release-0.23).
This means that:
- `@rerun-io/web-viewer@0.10.0` can only connect to a data source (`.rrd` file, gRPC connection, etc.) that originates from a Rerun SDK with version `0.10.0`!
- For versions after `@rerun-io/web-viewer@0.23.0`, the Viewer can load data from the previous _minor_ version of Rerun, e.g. `0.24` can load `0.23` files.

## Usage

The entrypoint for this packages is the [`WebViewer`](https://ref.rerun.io/docs/js/0.29.0-alpha.2/web-viewer/classes/WebViewer.html) class.
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
- A hosted `.rrd` file, such as <https://app.rerun.io/version/0.29.0-alpha.2/examples/dna.rrd>
- A gRPC connection to the SDK opened via the [`serve`](https://www.rerun.io/docs/reference/sdk/operating-modes#serve) API

If `rrd` is not set, the Viewer will display the same welcome screen as <https://app.rerun.io>.
This can be disabled by setting `hide_welcome_screen` to `true` in the options object of `viewer.start`.

⚠ It's important to set the viewer's width and height, as without it the viewer may not display correctly.
Setting the values to empty strings is valid, as long as you style the canvas through other means.

For a full example, see https://github.com/rerun-io/web-viewer-example.
You can open the example via CodeSandbox: https://codesandbox.io/s/github/rerun-io/web-viewer-example

ℹ️ Note:
This package only targets recent versions of browsers.
If your target browser does not support Wasm imports or top-level await, you may need to install additional plugins for your bundler.

For more information about using the package, visit:
- [Integration docs](https://rerun.io/docs/howto/integrations/embed-web#using-the-javascript-package).
- [Package docs](https://ref.rerun.io/docs/js/0.26.0/web-viewer/index.html).
