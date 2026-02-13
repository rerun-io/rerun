# re_arrow_combinators

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_arrow_combinators.svg)](https://crates.io/crates/re_arrow_combinators)
[![Documentation](https://docs.rs/re_arrow_combinators/badge.svg)](https://docs.rs/re_arrow_combinators)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Type-safe, composable transformations for Arrow arrays.

Provides building blocks for constructing complex data transformations through composition.
These transformations are designed to be used as primitives for user-defined functions (UDFs)
in query engines like DataFusion, as well as in SDK features like lenses.
