# re_types_builder_prelude

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

The vocabulary that Rerun's type definitions are written against: the `#[rerun_type]` attribute
macro, plus the handful of types that have no spelling in plain Rust (`f16`, `Binary`).

This crate exists to serve the definitions crate. It is not useful on its own.
