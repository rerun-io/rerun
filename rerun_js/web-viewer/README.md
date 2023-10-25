# Rerun Web Viewer

Embed the Rerun web viewer within your app.

<p align="center">
  <img width="800" alt="Rerun Viewer" src="https://github.com/rerun-io/rerun/assets/2624717/c4900538-fc3a-43b8-841a-8d226e7b5a2e">
</p>

## Install

```sh
npm i @rerun-io/web-viewer
```

## Example

```js
import { WebViewer } from "@rerun-io/web-viewer";

const URL = "...";
const parentElement = document.body;

const viewer = new WebViewer();
await viewer.start(URL, parentElement);

// Once you're done with it, call `stop`:
viewer.stop();
```

ℹ️ Note:
This package only targets recent browsers.
Your environment must support importing `.wasm` files as ES modules.

## Development

```
npm run build
```
