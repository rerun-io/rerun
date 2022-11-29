# run-wasm

This provides an [`xtask`](https://github.com/matklad/cargo-xtask) that makes it trivial to run web-based examples.

It relies on [`cargo-run-wasm`](https://github.com/rukai/cargo-run-wasm) to run the `wasm-bindgen` machinery, generate HTML files and all of that good stuff.
This is a temporary solution while we're in the process of building our own `xtask` tools.

You can try it e.g. with the standalone re_renderer demo:
```
cargo run-wasm --example 2d
```
