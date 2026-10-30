# re_protos

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_protos.svg)](https://crates.io/crates/re_protos)
[![Documentation](https://docs.rs/re_protos/badge.svg)](https://docs.rs/re_protos)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Rerun remote store node gRPC API service types (client and server).

This crate includes both the language-agnostic definitions (protobuf) as well as the generated code.

The Rust code is generated with `pixi run codegen-protos`.
The reflection descriptor set is compiled from the `.proto` sources into Cargo's `OUT_DIR` by `build.rs`, using the `protoc` bundled by `protoc-bin-vendored`.
No system `protoc` is needed for compilation.
It retains imported definitions and source comments, and is not generated for WebAssembly targets.
Cargo watches the schema directory, not the Rust sources, and unchanged descriptor bytes are not rewritten.

It also includes helpers for converting between the generated protobuf types and native Rust types.
