# Rerun Arrow Store

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_entity_db.svg)](https://crates.io/crates/re_entity_db)
[![Documentation](https://docs.rs/re_entity_db/badge.svg)](https://docs.rs/re_entity_db)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

[Apache Arrow](https://arrow.apache.org/) is a language-independent columnar memory format for arbitrary data.

The `re_arrow_store` crate is an in-memory time series database for Rerun log data. It is indexed by Entity path, component, timeline, and time. It supports out-of-order insertions, and fast `O(log(N))` queries.
