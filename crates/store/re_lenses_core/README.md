# re_lenses_core

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_lenses_core.svg)](https://crates.io/crates/re_lenses_core)
[![Documentation](https://docs.rs/re_lenses_core/badge.svg)](https://docs.rs/re_lenses_core)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Core lens types and composable Arrow array transformations.

This crate provides the Lenses definitions and builders, and composable
transformations for Arrow arrays. Transformations are composable operations
that convert one array type to another, preserving structural properties
like row counts and null handling.
