# re_view_web

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_view_web.svg?speculative-link)](https://crates.io/crates/re_view_web?speculative-link)
[![Documentation](https://docs.rs/re_view_web/badge.svg?speculative-link)](https://docs.rs/re_view_web?speculative-link)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

A view that shows a web page, using [`egui_webview`](https://github.com/rerun-io/egui_crates/tree/main/crates/egui_webview).
On the web it shows the page in an `<iframe>`.
Native builds don't show pages by default.
The native backend of `egui_webview` is CEF (Chromium), which downloads about 1 GB when it is built, so we plan to download CEF at run time instead.

To try the web view natively, build and run the viewer with CEF:

```sh
pixi run rerun-webview-cef
```

This adds CEF to `Cargo.lock`. Don't commit that.
