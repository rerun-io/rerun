# Rerun
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-MIT)
[![Apache](https://img.shields.io/badge/license-Apache-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-APACHE)

This repository contains the Rerun log SDK, server, and viewer.

# For our users
We don't have any pre-built binaries yet, so you need to build Rerun from source. There is some setup involved, but most of it should be pretty painless.

## Setup
* Install the Rust toolchain: <https://rustup.rs/>
* `git clone git@github.com:rerun-io/rerun.git && cd rerun`
* Run `./scripts/setup.sh`.
* Make sure `cargo --version` prints `1.63.0` once you are done

### Apple-silicon Macs
If you are using an Apple-silicon Mac, make sure `rustc -vV` outputs `host: aarch64-apple-darwin`. If not, this should fix it:

``` sh
rustup set default-host aarch64-apple-darwin && rustup install 1.63
```

## Installing the Rerun Viewer
After running the setup above, you can build and install the Rerun Viewer with:

```sh
cargo install --path ./crates/rerun/ --all-features
```

You should now be able to run `rerun --help` in any terminal.

## Build and install the Rerun Python SDK

```
python3 -m pip install --upgrade pip
pip3 install "./rerun_py"
```
Note: If you are unable to upgrade pip to version `>=21.3`, you need to pass `--use-feature=in-tree-build` to the `pip3 install` command.

### Run the example
```sh
example_car
```

### Using the Rerun Python SDK
See [`rerun_py/USAGE.md`](rerun_py/USAGE.md).


# Development
Take a look at [`CONTRIBUTING.md`](CONTRIBUTING.md).
