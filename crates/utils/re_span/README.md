# re_span

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_span.svg?sepculative-link)](https://crates.io/crates/re_span?sepculative-link)
[![Documentation](https://docs.rs/re_span/badge.svg)](https://docs.rs/re_span?sepculative-link)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

An integer range that always has a non-negative length.

The standard `std::ops::Range` can have `start > end`.
Taking a `Range` by argument thus means the callee must check for this eventuality and return an error.

In contrast, `Span` always has a non-negative length, i.e. `len >= 0`.
