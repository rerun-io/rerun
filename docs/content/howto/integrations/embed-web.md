---
title: Embed Rerun in Web pages
order: 100
---

Integrating the Rerun Viewer into your web application can be accomplished either by [utilizing an iframe](#embedding-apprerunio-using-an-iframe) or by using our [JavaScript package](#using-the-javascript-package).

## Embedding `app.rerun.io` using an `<iframe>`

This approach is straightforward and requires minimal setup. However, the drawback is that it lacks programmable control over the web viewer.

```html
<iframe src="https://app.rerun.io/version/{RERUN_VERSION}/index.html?url={RRD_URL}"></iframe>
```

To implement this, fill in the placeholders:
- `RRD_URL` - The URL of the recording to display in the viewer.
- `RERUN_VERSION` - The version of the Rerun SDK used to generate the recording.

The `RRD_URL` can be a file served over `http` (e.g. `https://app.rerun.io/version/0.20.3/examples/arkit_scenes.rrd`), or a connection to an SDK using our [serve](https://www.rerun.io/docs/reference/sdk/operating-modes#serve) API (e.g. `rerun+http://localhost:4321/proxy`).

For instance:

```html
<iframe src="https://app.rerun.io/version/0.20.3/?url=https://app.rerun.io/version/0.20.3/examples/arkit_scenes.rrd"></iframe>
```

## Using the JavaScript package

We offer JavaScript bindings to the Rerun Viewer via NPM. This method provides control over the Viewer but requires a JavaScript web application setup with a bundler.

Various packages are available:
- [@rerun-io/web-viewer](https://www.npmjs.com/package/@rerun-io/web-viewer): Suitable for JS apps without a framework or frameworks without dedicated packages.
- [@rerun-io/web-viewer-react](https://www.npmjs.com/package/@rerun-io/web-viewer-react): Designed specifically for React apps.

> ℹ️ Note: The stability of the `rrd` format is still evolving, so the package version corresponds to the supported Rerun SDK version. Therefore, `@rerun-io/web-viewer@0.10.0` can only connect to a data source (`.rrd` file, WebSocket connection, etc.) originating from a Rerun SDK with version `0.10.0`!

### Basic example

To begin, install the package ([@rerun-io/web-viewer](https://www.npmjs.com/package/@rerun-io/web-viewer)) from NPM:

```
npm i @rerun-io/web-viewer
```

> ℹ Note: This package is compatible only with recent browser versions. If your target browser lacks support for Wasm imports or top-level await, additional plugins may be required for your bundler setup. For instance, if you're using [Vite](https://vitejs.dev/), you'll need to install [vite-plugin-wasm](https://www.npmjs.com/package/vite-plugin-wasm) and [vite-plugin-top-level-await](https://www.npmjs.com/package/vite-plugin-top-level-await) and integrate them into your `vite.config.js`.

Once installed and configured, import and use it within your application:

```js
import { WebViewer } from "@rerun-io/web-viewer";

const rrdUrl = null;
const parentElement = document.body;

const viewer = new WebViewer();
await viewer.start(rrdUrl, parentElement);
```

The Viewer creates a `<canvas>` on the provided `parentElement` and executes within it.

The first argument for `start` determines the recordings to open in the viewer. It can be:
- `null` for an initially empty viewer
- a URL string to open a single recording
- an array of strings to open multiple recordings

Each URL can be either a file served over `http` or a connection to an SDK using our [serve](https://www.rerun.io/docs/reference/sdk/operating-modes#serve) API. See [web-viewer-serve-example](https://github.com/rerun-io/web-viewer-serve-example) for a full example of how to log data from our Python SDK to an embedded Rerun Viewer.

### Controlling the canvas

By default, the web viewer attempts to expand the canvas to occupy all available space. You can customize its dimensions by placing it within a container:

```html,id=embed-web-viewer-canvas-control-html
<body>
  <div id="viewer-container"></div>
</body>
```

```css,id=embed-web-viewer-canvas-control-css
#viewer-container {
  position: relative;
  height: 640px;
  width: 100%;
}
```

```js,id=embed-web-viewer-canvas-control-js
const parentElement = document.getElementById("viewer-container");

const viewer = new WebViewer();
await viewer.start(null, parentElement);
```

### Viewer API

The Viewer API supports adding and removing recordings:

```js,id=embed-web-viewer-api-js-open-close
const rrdUrl = "https://app.rerun.io/version/0.20.3/examples/arkit_scenes.rrd";

// Open a recording:
viewer.open(rrdUrl);

// Later on…
viewer.close(rrdUrl);
```

Once finished with the Viewer, you can stop it and release all associated resources:

```js,id=embed-web-viewer-api-js-stop
viewer.stop();
```

This action also removes the canvas from the page.

You can `start` and `stop` the same `WebViewer` instance multiple times.

### Callbacks

The Viewer API also allows registering callbacks for certain events.

For example, here is how you would react to entities being selected in the Viewer:
```js
viewer.on("selectionchange", (items) => {
  for (const item of items) {
    if (item.kind === "entity") {
      console.log(item.entity_path);
    }
  }
});

viewer.on("timeupdate", (time) => {
  console.log("current time", time);
});

viewer.on("timelinechange", (timeline, time) => {
  console.log("timeline changed to: ", timeline);
  console.log("current time", time);
})
```
