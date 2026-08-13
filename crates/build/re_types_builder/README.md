# re_types_builder

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_types_builder.svg)](https://crates.io/crates/re_types_builder)
[![Documentation](https://docs.rs/re_types_builder/badge.svg)](https://docs.rs/re_types_builder)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

This crate implements Rerun's code generation tools.

These tools translate the type definitions in `re_type_definitions` — a subset of Rust — into code.

You can generate the code with `pixi run codegen`.

### Doclinks

The definitions can contain rustdoc links (`///`) to Rerun types and their fields or enum variants.
Use fully-qualified paths, such as [`rerun::archetypes::Image`] or [`rerun::components::FillMode::DenseWireframe`].

Rustdoc checks the links in the definitions.
Codegen translates them into links for each target language.
