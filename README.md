# Rerun
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-MIT)
[![Apache](https://img.shields.io/badge/license-Apache-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-APACHE)

This repository contains the Rerun log SDK, server, and viewer.

# For our users
We don't have any pre-built binaries yet, so you need to build Rerun from source. There is some setup involved, but most of it should be pretty painless.

## Setup
First up, you need to install the Rust toolchain: https://rustup.rs/

Then, setup the rest of the required tools by running `./scripts/setup.sh`.

## Installing the Rerun viewer
After running the setup above, you can build and install the rerun viewer with:

```sh
cargo install --path ./crates/rerun/ --all-features
```

You should now be able to run `rerun --help` in any terminal.

## Build and install the Rerun Python SDK

```
python3 -m pip install --upgrade pip
pip3 install "crates/re_sdk_python[tests,examples]"
```
Note: If you are unable to upgrade pip to version `>=21.3`, you need to pass `--use-feature=in-tree-build` to the `pip3 install` command.

### Run the example
```sh
python3 crates/re_sdk_python/example.py
```

### Using the Rerun Python SDK
See [`crates/re_sdk_python/USAGE.md`](crates/re_sdk_python/USAGE.md).


# Development
Take a look at [`CONTRIBUTING.md`](CONTRIBUTING.md).
