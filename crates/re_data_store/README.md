# Rerun Data Store

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_data_store.svg)](https://crates.io/crates/re_data_store)
[![Documentation](https://docs.rs/re_data_store/badge.svg)](https://docs.rs/re_data_store)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

[Apache Arrow](https://arrow.apache.org/) is a language-independent columnar memory format for arbitrary data.

The `re_data_store` crate is an in-memory time series database for Rerun log data. It is indexed by Entity path, component, timeline, and time. It supports out-of-order insertions, and fast `O(log(N))` queries.
