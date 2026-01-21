# re_memory_view

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_memory_view.svg?speculative-link)](https://crates.io/crates/re_memory_view?speculative-link)
[![Documentation](https://docs.rs/re_memory_view/badge.svg?speculative-link)](https://docs.rs/re_memory_view?speculative-link)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Flamegraph visualization for memory usage trees.

This crate provides an interactive flamegraph widget for visualizing `MemUsageTree` structures from re_byte_size.

## Running the demo

To see the flamegraph in action, run the demo application:

```bash
cargo run --example demo -p re_memory_view
```

The demo creates a sample memory hierarchy showing various subsystems (viewer, store, cache, etc.)
and allows you to interact with the flamegraph visualization.
