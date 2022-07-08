# Rerun
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-MIT)
[![Apache](https://img.shields.io/badge/license-Apache-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-APACHE)

This repository contains the Rerun log SDK, server, and viewer.

## Setup
Install Rust: https://rustup.rs/

``` sh
./setup.sh
```

## Check
``` sh
./check.sh
```

## Structure
The main crates are found in the `crates/` folder, with examples in the `examples/` folder.

Read about individual examples for details on how to run them.

To learn about the viewer, run:

```
cargo run --release -p re_viewer -- --help
```


### Other
You can view higher log levels with `export RUST_LOG=debug` or `export RUST_LOG=trace`.
