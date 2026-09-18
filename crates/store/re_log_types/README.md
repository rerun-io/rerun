# re_log_types

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_log_types.svg)](https://crates.io/crates/re_log_types)
[![Documentation](https://docs.rs/re_log_types/badge.svg)](https://docs.rs/re_log_types)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

The basic building blocks of the Rerun log format: entity paths, timelines, and store ids.

An entity path names *what* was logged and a `TimePoint` on one or more timelines names *when*.
The messages that carry them between the SDK, an `.rrd` file, and the viewer live in [`re_log_msg`](../re_log_msg/README.md), and the data itself is described by [`re_sdk_types`](../re_sdk_types/README.md).
