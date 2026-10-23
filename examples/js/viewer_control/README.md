# Viewer control

Control playback of the Rerun web viewer from TypeScript with [`@rerun-io/web-viewer`](../../../rerun_js/web-viewer/), take screenshots of it, and listen to its events.
See [`src/main.ts`](src/main.ts), and the [Web Viewer API reference](https://ref.rerun.io/docs/js/) for everything else the viewer can do.

```sh
pixi run js-example-viewer-control       # Build the local web-viewer and start the dev server
pixi run js-example-viewer-control-test  # Playwright smoke test (also runs on CI)
```

Outside of this repository, install `@rerun-io/web-viewer` from npm instead.
