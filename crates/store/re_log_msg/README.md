# re_log_msg

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_log_msg.svg)](https://crates.io/crates/re_log_msg?speculative-link)
[![Documentation](https://docs.rs/re_log_msg/badge.svg)](https://docs.rs/re_log_msg?speculative-link)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

The messages that carry Rerun data between the SDK, `.rrd` files and the viewer.

A `LogMsg` is the envelope for a recording or blueprint: it announces a store with a `StoreInfo`, carries one chunk at a time as an `ArrowMsg`, and activates a fully transmitted blueprint.
A `TableMsg` carries a standalone table, which is never stored in an `.rrd` file.
The building blocks these messages are made of — entity paths, timelines, store ids — live in [`re_log_types`](../re_log_types/README.md).
