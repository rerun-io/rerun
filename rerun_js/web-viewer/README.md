# Rerun Web Viewer

Embed the Rerun web viewer within your app.

<p align="center">
  <img width="800" alt="Rerun Viewer" src="https://github.com/rerun-io/rerun/assets/2624717/c4900538-fc3a-43b8-841a-8d226e7b5a2e">
</p>

## Install

```sh
npm i @rerun-io/web-viewer
```

## Usage

The web viewer is an object which manages a canvas element:

```js
import { WebViewer } from "@rerun-io/web-viewer";

const URL = "…";
const parentElement = document.body;

const viewer = new WebViewer();
await viewer.start(URL, parentElement);

// Once you're done with it, call `stop`:
viewer.stop();
```

You can style this canvas element however you wish.

For a live example, see https://github.com/rerun-io/web-viewer-example.

ℹ️ Note:
This package only targets recent versions of browsers.
If your target browser does not support Wasm imports or top-level await,
you may need to install additional plugins for your bundler.

## Development

```
npm run build
```
