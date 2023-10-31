# Rerun Web Viewer

Embed the Rerun web viewer within your React app.

<p align="center">
  <img width="800" alt="Rerun Viewer" src="https://github.com/rerun-io/rerun/assets/2624717/c4900538-fc3a-43b8-841a-8d226e7b5a2e">
</p>

## Install

```
$ npm i @rerun-io/web-viewer-react
```

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
