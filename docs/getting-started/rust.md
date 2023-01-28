---
title: For Rust Users
order: 4
---

Much of our getting-started documentation is tailored toward python.

Even for Rust users, the Python [quick tour](quick-tour) is a good way to get an overview of the core
functionality. The viewer is the same regardless which language you use to log data.

In rust, the rerun visualizer and the logging library both accessed via the [rerun](https://crates.io/crates/rerun)
crate.

Install rerun viewer:
```bash
$ cargo install rerun
```

Add rerun to your `Cargo.toml`
```bash
$ cargo add rerun
```

And a truly minimal logging application:
```rust
use rerun;

fn main() {
    rerun.init("rust_example");
    rerun.log_points("points", ...);
}
```

For more on using the Rerun viewer, checkout the [quick tour](getting-started/quick-tour) or the
[viewer reference](reference/viewer).

Or, to find out about how to log data with Rerun see [Logging Data from Rust](getting-started/logging-data-rust)