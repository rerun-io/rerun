# run-wasm

This provides an [`xtask`](https://github.com/matklad/cargo-xtask) that makes it trivial to run web-based examples.
We currently use it only for the samples of `re_renderer`.

It relies on [`cargo-run-wasm`](https://github.com/rukai/cargo-run-wasm) to run the `wasm-bindgen` machinery, generate HTML files and all of that good stuff.
This is a temporary solution while we're in the process of building our own `xtask` tools.

Example of running a `re_renderer` demo with `run-wasm`:
```
cargo run-wasm --example 2d
```
