# `WebViewer` API tests

Playwright tests for every part of the [`@rerun-io/web-viewer`](../../../rerun_js/web-viewer/) API, against a local build of the web viewer.
For a short introduction to the API, see the [`viewer_control` example](../../../examples/js/viewer_control/) instead.

```sh
pixi run js-viewer-control-tests      # Run the tests (also runs on CI). Needs a built web-viewer.
pixi run js-viewer-control-test-page  # Build the web-viewer and open the test page, to try the API by hand
```

The test page loads `examples/assets/example.rrd`, or `?url=…`.
Pick a call from the list, edit it if you like, and run it to see the result.
The tests drive the same page through `window.viewer`.
