# Rerun Web Viewer

Embed the Rerun web viewer within your React app.

<p align="center">
  <img width="800" alt="Rerun Viewer" src="https://github.com/rerun-io/rerun/assets/2624717/c4900538-fc3a-43b8-841a-8d226e7b5a2e">
</p>

## Install

```
$ npm i @rerun-io/web-viewer-react
```

ℹ️ Note:
The package version is equal to the supported rerun SDK version.
This means that `@rerun-io/web-viewer-react@0.10.0` can only connect to a data source (`.rrd` file, websocket connection, etc.) that originates from a rerun SDK with version `0.10.0`!

## Usage

```jsx
import WebViewer from "@rerun-io/web-viewer-react";

export default function App() {
  return (
    <div>
      <WebViewer rrd="...">
    </div>
  )
}
```

ℹ️ Note:
This package only targets recent versions of browsers.
If your target browser does not support Wasm imports, you may need to install additional plugins for your bundler.

## Development

```
$ npm run build
```
