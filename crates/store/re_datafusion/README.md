# re_datafusion

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_datafusion.svg)](https://crates.io/crates/re_datafusion)
[![Documentation](https://docs.rs/re_datafusion/badge.svg)](https://docs.rs/re_datafusion)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

DataFusion interfaces to Rerun gRPC queries

See [`PIPELINE_BUDGET.md`](PIPELINE_BUDGET.md) for an explanation of the memory backpressure mechanism used by `SegmentStreamExec` to bound RAM across the IO → channel → in-memory store pipeline, and why a byte-bounded channel is not sufficient.
