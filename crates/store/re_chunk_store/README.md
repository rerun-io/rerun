# Rerun chunk store

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_chunk_store.svg)](https://crates.io/crates/re_chunk_store?speculative-link)
[![Documentation](https://docs.rs/re_chunk_store/badge.svg)](https://docs.rs/re_chunk_store?speculative-link)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

[Apache Arrow](https://arrow.apache.org/) is a language-independent columnar memory format for arbitrary data.

The `re_chunk_store` crate is an in-memory time series database for Rerun log data. It is indexed by Entity path, component, timeline, and time. It supports out-of-order insertions, and fast `O(log(N))` queries.
