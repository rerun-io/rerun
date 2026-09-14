# re_log_types

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_log_types.svg)](https://crates.io/crates/re_log_types)
[![Documentation](https://docs.rs/re_log_types/badge.svg)](https://docs.rs/re_log_types)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

The basic building blocks of the Rerun log format: entity paths, timelines, store ids, and log messages.

An entity path names *what* was logged, a `TimePoint` on one or more timelines names *when*, and a `LogMsg` is the envelope that carries it between the SDK, an `.rrd` file, and the viewer.
The data itself is described by [`re_sdk_types`](../re_sdk_types/README.md).
