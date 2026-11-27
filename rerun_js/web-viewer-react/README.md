<h1 align="center">
  <a href="https://www.rerun.io/">
    <img width="1000" height="200" alt="Banner with Rerun logo" src="https://static.rerun.io/d0f5443d4803cac65c73fcc064936c09f5e7f208_rerun_banner.png" />
  </a>
</h1>

<p align="center">
  <a href="https://www.npmjs.com/package/@rerun-io/web-viewer-react"><img alt="Latest version" src="https://img.shields.io/npm/v/@rerun-io/web-viewer-react.svg"></a>
  <a href="https://github.com/rerun-io/rerun/blob/main/LICENSE-MIT"><img alt="MIT" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
  <a href="https://discord.gg/Gcm8BbTaAj"><img alt="Rerun Discord" src="https://img.shields.io/discord/1062300748202921994?label=Rerun%20Discord"></a>
</p>

# Rerun web viewer

Rerun is the data layer for physical AI: it logs, stores, queries, and visualizes multi-rate, multimodal data.
This package embeds the Rerun Viewer in a React app.

It wraps the framework-agnostic [`@rerun-io/web-viewer`](https://www.npmjs.com/package/@rerun-io/web-viewer) package, and ships with TypeScript types.

## Install

```sh
npm i @rerun-io/web-viewer-react
```

## Example

```jsx
import WebViewer from "@rerun-io/web-viewer-react";

export default function App() {
  return <WebViewer width="800px" height="600px" rrd="…" />;
}
```

<p align="center">
  <picture>
    <img src="https://static.rerun.io/opf_screenshot/bee51040cba93c0bae62ef6c57fa703704012a41/full.png" alt="The Rerun Viewer showing a 3D reconstruction">
    <source media="(max-width: 480px)" srcset="https://static.rerun.io/opf_screenshot/bee51040cba93c0bae62ef6c57fa703704012a41/480w.png">
    <source media="(max-width: 768px)" srcset="https://static.rerun.io/opf_screenshot/bee51040cba93c0bae62ef6c57fa703704012a41/768w.png">
    <source media="(max-width: 1024px)" srcset="https://static.rerun.io/opf_screenshot/bee51040cba93c0bae62ef6c57fa703704012a41/1024w.png">
    <source media="(max-width: 1200px)" srcset="https://static.rerun.io/opf_screenshot/bee51040cba93c0bae62ef6c57fa703704012a41/1200w.png">
  </picture>
</p>

Always set `width` and `height`, or the Viewer may not display correctly.
Empty strings are valid if you size the canvas with your own CSS.

## Data sources

`rrd` is a URL that points to one of:
- A hosted `.rrd` file, such as <https://app.rerun.io/version/0.38.1/examples/dna.rrd>
- A gRPC server started with the SDK's [`serve_grpc`](https://rerun.io/docs/log/sinks-and-modes/operating-modes#serve_grpc) API, such as `rerun+http://127.0.0.1:9876/proxy`

Without an `rrd`, the Viewer shows the same welcome screen as <https://app.rerun.io>.
To hide it, set the `hideWelcomeScreen` prop to `true`.

## Events

Each Viewer event is available as an `on…` prop:

```jsx
<WebViewer
  width="800px"
  height="600px"
  rrd="…"
  onSelectionChange={(event) => console.log(event.items)}
  onTimeUpdate={(event) => console.log(event.time)}
/>
```

## Usage data

By default, the Viewer sends usage data to Rerun, and shows a one-time notice about it.
Nothing is sent during a browser's first visit, so users can turn it off first.
To send nothing, set the `analytics` prop to `"off"`:

```jsx
<WebViewer width="800px" height="600px" rrd="…" analytics="off" />
```

Users can always turn usage data off in the Viewer's settings, which overrides `"on"`.
If your app leaves usage data on, consider mentioning Rerun in your privacy policy.
See the [Rerun privacy policy](https://rerun.io/privacy) for what is collected.

## Compatibility

The package version matches the Rerun SDK version.
The Viewer reads `.rrd` files written by the same SDK version, and by any older version from `0.23` on.
A Viewer is not guaranteed to read data from a newer SDK, so keep the package at least as new as the SDK that writes your data.
See [RRD file backwards compatibility](https://rerun.io/docs/log/tutorial#rrd-file-backwards-compatibility) for details.

The package targets recent browsers.
If your target browser does not support Wasm imports or top-level await, your bundler may need extra plugins.
For example, Vite needs [`vite-plugin-wasm`](https://www.npmjs.com/package/vite-plugin-wasm) and [`vite-plugin-top-level-await`](https://www.npmjs.com/package/vite-plugin-top-level-await).

## Resources

- [Embedding guide](https://rerun.io/docs/viewer/integrations/embed-web#using-the-javascript-package)
- [API reference](https://ref.rerun.io/docs/js/0.38.1/web-viewer-react/index.html)
- [Troubleshooting](https://rerun.io/docs/getting-started/install-rerun/troubleshooting)
- [Discord](https://discord.gg/Gcm8BbTaAj)
