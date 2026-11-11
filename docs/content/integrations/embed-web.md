---
title: Embed Rerun in Web pages
order: 100
description: Embed the web Viewer in a page with an iframe, the JavaScript package, or Gradio
---

For the full JavaScript API, see the [Web Viewer API reference](https://ref.rerun.io/docs/js/).

Integrating the Rerun Viewer into your web application can be accomplished either by [utilizing an iframe](#embedding-apprerunio-using-an-iframe) or by using our [JavaScript package](#using-the-javascript-package).

## Embedding `app.rerun.io` using an `<iframe>`

This approach is straightforward and requires minimal setup. However, the drawback is that it lacks programmable control over the web viewer.

```html
<iframe src="https://app.rerun.io/version/{RERUN_VERSION}/index.html?url={RRD_URL}"></iframe>
```

To implement this, fill in the placeholders:
- `RRD_URL` - The URL of the recording to display in the viewer, URL-encoded.
- `RERUN_VERSION` - A Rerun release, such as `0.38.1`, or `latest` for the newest release.

The `RRD_URL` can be a file served over `http` (e.g. `https://app.rerun.io/version/latest/examples/dna.rrd`), or a connection to an SDK using our [`serve_grpc`](../logging-and-ingestion/sinks-and-modes/operating-modes.md#serve_grpc) API (e.g. `rerun+http://localhost:9876/proxy`).
See [Open a file by URL](../viewer/extend/urls.md#open-a-file-by-url) for URL-encoding and the CORS headers that the file host must send.
For gRPC connections, see [Connecting to a gRPC server from another origin](#connecting-to-a-grpc-server-from-another-origin-cors).

For instance:

```html
<iframe src="https://app.rerun.io/version/latest/?url=https%3A%2F%2Fapp.rerun.io%2Fversion%2Flatest%2Fexamples%2Fdna.rrd"></iframe>
```

Choose a Viewer version that can read your recordings.
RRD files written by Rerun 0.23 or later can be opened by newer Viewers, but compatibility is only guaranteed across adjacent minor versions (see [RRD file backwards compatibility](../logging-and-ingestion/tutorial.md#rrd-file-backwards-compatibility)).
`latest` changes with every release, so pin a version if the files you show will not be regenerated.

### Matching the host page's theme

By default, the embedded viewer follows the user's OS theme (`prefers-color-scheme`). If your host page has its own theme toggle, you can pin the viewer to match by passing `theme=dark`, `theme=light`, or `theme=system`:

```html
<iframe src="https://app.rerun.io/version/{RERUN_VERSION}/?url={RRD_URL}&theme=dark"></iframe>
```

This is useful for sites whose theme can differ from the OS preference — without it, a user on a light-mode OS visiting your dark-mode page would see a bright viewer panel against a dark background.

## Using the JavaScript package

We offer JavaScript bindings to the Rerun Viewer via NPM. This method provides control over the Viewer but requires a JavaScript web application setup with a bundler.

Various packages are available:
- [@rerun-io/web-viewer](https://www.npmjs.com/package/@rerun-io/web-viewer): Suitable for JS apps without a framework or frameworks without dedicated packages.
- [@rerun-io/web-viewer-react](https://www.npmjs.com/package/@rerun-io/web-viewer-react): Designed specifically for React apps.

> [!NOTE]
> The package version matches the Rerun SDK version.
> The Viewer in the package reads data from the same version, and from older versions as described in [RRD file backwards compatibility](../logging-and-ingestion/tutorial.md#rrd-file-backwards-compatibility).
> To be safe, use the same version for the package and for the SDK that writes your data.

### Basic example

To begin, install the package ([@rerun-io/web-viewer](https://www.npmjs.com/package/@rerun-io/web-viewer)) from NPM:

```
npm i @rerun-io/web-viewer
```

> [!NOTE]
> This package is compatible only with recent browser versions. If your target browser lacks support for Wasm imports or top-level await, additional plugins may be required for your bundler setup. For instance, if you're using [Vite](https://vitejs.dev/), you'll need to install [vite-plugin-wasm](https://www.npmjs.com/package/vite-plugin-wasm) and [vite-plugin-top-level-await](https://www.npmjs.com/package/vite-plugin-top-level-await) and integrate them into your `vite.config.js`.

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

Each URL can be either a file served over `http` or a connection to an SDK using our [`serve_grpc`](../logging-and-ingestion/sinks-and-modes/operating-modes.md#serve_grpc) API. See [web-viewer-serve-example](https://github.com/rerun-io/web-viewer-serve-example) for a full example of how to log data from our Python SDK to an embedded Rerun Viewer.

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
const rrdUrl = "https://app.rerun.io/version/latest/examples/dna.rrd";

// Open a recording, and wait for it to load:
const { storeIds } = await viewer.open(rrdUrl);

// Later on…
await viewer.close(rrdUrl);
```

Once finished with the Viewer, you can stop it and release all associated resources:

```js,id=embed-web-viewer-api-js-stop
viewer.stop();
```

This action also removes the canvas from the page.

You can `start` and `stop` the same `WebViewer` instance multiple times.

### Reading and driving the Viewer

The Viewer can report what it shows, and be driven from code:

```js,id=embed-web-viewer-control
// What the Viewer shows: the open recordings, their timelines, and time cursors.
const state = await viewer.getState();

// On a duration timeline, seek the active recording to one second, and play from there.
await viewer.setTime({ durationNanos: 1_000_000_000n, playState: "playing" });

// Show another open recording.
await viewer.setLocation(state.recordings[0].location);
```

Each of these methods returns a promise, because the Viewer handles each request on its next frame.
Some requests take even longer, such as a screenshot, or opening a URL, which waits for the data to load.
They are all async so that they work the same way.

A failed call rejects with a `ViewerControlError`, whose `code` is a gRPC status code such as `NOT_FOUND`.
This API is experimental, and may change between releases.

For a runnable TypeScript example that controls playback and listens to Viewer events, see [`examples/js/viewer_control`](https://github.com/rerun-io/rerun/tree/main/examples/js/viewer_control).

### Callbacks

The Viewer API also allows registering callbacks for certain events.

For example, here is how you would react to entities being selected in the Viewer:
```js
viewer.on("selection_change", (event) => {
  for (const item of event.items) {
    if (item.type === "entity") {
      console.log(item.entityPath);
    }
  }
});
```

## Connecting to a gRPC server from another origin (CORS)

A web Viewer that connects to a gRPC server (`serve_grpc`, `rerun --serve-grpc`, or `rerun server`) sends its requests from the origin of the page it runs on.
By default, the server only accepts requests from pages on `localhost`, `127.0.0.1`, and `rerun.io` (including its subdomains, such as `app.rerun.io`).
If your page is served from another origin, allow it on the server:

```python
rr.serve_grpc(cors_allow_origin=["https://my-app.example.com"])
```

```sh
rerun --serve-grpc --cors-allow-origin "https://my-app.example.com"
rerun server --cors-allow-origin "https://my-app.example.com"
```

Patterns can use `*` as a wildcard, for example `"https://*.example.com"` or `"https://example.com:*"`.

This is for gRPC servers. For `.rrd` files that you load over `http`, the file host must allow your page's origin instead (see [CORS for files](../viewer/extend/urls.md#cors)).

## Authentication

The Viewer authenticates to Rerun servers with a bearer token (a JWT).
In the JavaScript package, pass it as the `fallback_token` option of `start`, or set it later with `viewer.set_credentials(access_token, email)`.
The `login` option enables the OAuth login UI for Rerun Hub.

You cannot add arbitrary HTTP headers, such as cookies or custom auth headers, to the Viewer's requests (see [missing features](../development/roadmap.md#missing-features)).
To show files that need other credentials, give the Viewer presigned URLs instead.

## Memory and performance

The web Viewer is the same Viewer as the native one, compiled to WebAssembly.
It runs in the browser tab, so it is slower than the native Viewer, and has less memory: a WebAssembly process can use at most 4 GiB, and the Viewer limits itself to 2.5 GB by default.
Use the `memory_limit` option of `start` to lower that limit.
When the limit is reached, the Viewer drops the oldest data.

## Gradio

To show the Viewer in a [Gradio](https://www.gradio.app/) app, use the [`gradio_rerun`](https://github.com/rerun-io/gradio-rerun-viewer) component:

```sh
pip install gradio_rerun
```

It embeds the web Viewer, so the memory and performance limits above apply.
See its README for examples of streaming data to it from a Gradio app.
