# Rerun chunk store

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_compute_graph.svg)](https://crates.io/crates/re_compute_graph)
[![Documentation](https://docs.rs/re_compute_graph/badge.svg)](https://docs.rs/re_compute_graph)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

[Apache Arrow](https://arrow.apache.org/) is a language-independent columnar memory format for arbitrary data.

The `re_compute_graph` crate is an in-memory time series database for Rerun log data. It is indexed by Entity path, component, timeline, and time. It supports out-of-order insertions, and fast `O(log(N))` queries.
