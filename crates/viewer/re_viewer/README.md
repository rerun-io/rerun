# The Rerun Viewer

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_viewer.svg)](https://crates.io/crates/viewer/re_viewer)
[![Documentation](https://docs.rs/re_viewer/badge.svg)](https://docs.rs/re_viewer)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

This is the main crate with all the GUI.

This can be compiled as a web-app by building for Wasm. To run it natively, use the `rerun` binary.

Talks to the server over WebSockets (using `re_ws_comms`).
