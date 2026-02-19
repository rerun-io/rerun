# re_backoff

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_arrow_combinators.svg)](https://crates.io/crates/re_arrow_combinators)
[![Documentation](https://docs.rs/re_arrow_combinators/badge.svg)](https://docs.rs/re_arrow_combinators)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Implements utility code to help with backoff and retry logic.

### Why not use existing traits like backoff, backon, tokio-retry2, or tower(retry)?

The code is small and simple, that it feels unnecessary to add an external dependency for it. We should re-evaluate should this become ever-complicated.
