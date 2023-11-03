# Rerun Web Viewer

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

## Install

```
$ npm i @rerun-io/web-viewer
```

ℹ️ Note:
The package version is equal to the supported rerun SDK version.
This means that `@rerun-io/web-viewer@0.10.0` can only connect to a data source (`.rrd` file, websocket connection, etc.) that originates from a rerun SDK with version `0.10.0`!

## Usage

The web viewer is an object which manages a canvas element:

```js
import { WebViewer } from "@rerun-io/web-viewer";

const URL = "…";
const parentElement = document.body;

const viewer = new WebViewer();
await viewer.start(URL, parentElement);
// …
viewer.stop();
```

You can style this canvas element however you wish.

For a live example, see https://github.com/rerun-io/web-viewer-example.

ℹ️ Note:
This package only targets recent versions of browsers.
If your target browser does not support Wasm imports, you may need to install additional plugins for your bundler.

## Development

```
$ npm run build
```
