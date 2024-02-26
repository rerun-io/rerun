# re_web_viewer_server

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_web_viewer_server.svg)](https://crates.io/crates/re_web_viewer_server)
[![Documentation](https://docs.rs/re_web_viewer_server/badge.svg)](https://docs.rs/re_web_viewer_server)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Serves the Rerun web viewer (`re_viewer` as Wasm and HTML) over HTTP.

When developing, you must run `cargo r -p re_build_web_viewer -- --debug` (or `--release`), before building this package.
This is done automatically with `pixi run rerun-web`.
