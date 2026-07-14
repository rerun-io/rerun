# re_web_viewer_server

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_web_viewer_server.svg)](https://crates.io/crates/re_web_viewer_server)
[![Documentation](https://docs.rs/re_web_viewer_server/badge.svg)](https://docs.rs/re_web_viewer_server)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Serves the Rerun web viewer (`re_viewer` as Wasm and HTML) over HTTP.

When developing, you must run `pixi run rerun-build-web` (or `pixi run rerun-build-web-release`), before building this package.
This is done automatically with `pixi run rerun-web`.

## Embedding modes

By default, web viewer assets are embedded at compile time using `include_bytes!`.

When built with `RERUN_TRAILING_WEB_VIEWER=1`, the assets are expected to be appended to the binary via a post-processing step using `scripts/append_web_viewer.py`. This allows parallel building of CLI and web viewer in CI. Binaries built this way will panic if used before the post-processing step completes.
