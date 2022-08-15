# Rerun
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-MIT)
[![Apache](https://img.shields.io/badge/license-Apache-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-APACHE)

This repository contains the Rerun log SDK, server, and viewer.

# For our users
We don't have any pre-built binaries yet, so you need to build Rerun from source. There is some setup involved, but most of it should be pretty painless.

First up, you need to install the Rust toolchain: https://rustup.rs/

Then, setup the rest of the required tools by running `./setup.sh`.

## Installing the Rerun viewer
After running the setup above, you can build and install the rerun viewer with:

```sh
cargo install --path ./crates/rerun/ --all-features
```

You should now be able to run `rerun --help` in any terminal.

## Installing the Rerun Python SDK

First build it:
```
source crates/re_sdk_python/setup_env.sh
maturin build -m ./crates/re_sdk_python/Cargo.toml --release
```

Now you can install `rerun` in any Python3 environment using:

```
pip3 install PATH_TO_RERUN_REPOSITORY/target/wheels/*.whl
```

### Using the Rerun Python SDK
See [`crates/re_sdk_python/USAGE.md`](crates/re_sdk_python/USAGE.md).


# Development
Take a look at [`CONTRIBUTING.md`](CONTRIBUTING.md).
