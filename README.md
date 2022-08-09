# Rerun
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-MIT)
[![Apache](https://img.shields.io/badge/license-Apache-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-APACHE)

This repository contains the Rerun log SDK, server, and viewer.

## Setup
Install Rust: https://rustup.rs/

``` sh
./setup.sh
```

## Installation
After running the setup above, you can install the rerun viewer with:

```sh
cargo install --path crates/rerun --all-features
```

You should now be able to run `rerun --help`.


# Development

## Structure
The main crates are found in the `crates/` folder, with examples in the `examples/` folder.

Read about individual examples for details on how to run them.

To learn about the viewer, run:

```
cargo run --release -p rerun -- --help
```

### Other
You can view higher log levels with `export RUST_LOG=debug` or `export RUST_LOG=trace`.


## Linting
We use [cargo cranky](https://github.com/ericseppanen/cargo-cranky) to specify our clippy lints in `Cranky.toml`. Usage: `cargo cranky`.

We use [cargo deny](https://github.com/EmbarkStudios/cargo-deny) to check our dependency tree for copy-left licenses, duplicate dependencies and [rustsec advisories](https://rustsec.org/advisories). You can configure it in `deny.toml`. Usage: `cargo deny check`.

Configure your editor to run `cargo fmt` on save. Also configure it to strip trailing whitespace, an to end each file with a newline. Settings for VSCode can be found in the `.vscode` folder and should be applied automatically. If you are using another editor, consider adding good setting to this repository!

To check everything in one go, run `./check.sh`. `check.sh` should ideally check approximately the same things as our CI.

### Optional
You can use [bacon](https://github.com/Canop/bacon) to automatically check your code on each save. For instance, running just `bacon` will re-run `cargo cranky` each time you change a rust file. See `bacon.toml` for more.
