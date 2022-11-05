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
* Make sure `cargo --version` prints `1.65.0` once you are done

### Apple-silicon Macs
If you are using an Apple-silicon Mac, make sure `rustc -vV` outputs `host: aarch64-apple-darwin`. If not, this should fix it:

``` sh
rustup set default-host aarch64-apple-darwin && rustup install 1.65
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
python examples/car/main.py
```

### Using the Rerun Python SDK
See [`rerun_py/USAGE.md`](rerun_py/USAGE.md).


# Development
Take a look at [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Improving compile times

As of today, we link everything statically in both debug and release builds, which makes custom linkers and split debuginfo the two most impactful tools we have at our disposal in order to improve compile times.

These tools can configured through your `Cargo` configuration, available at `$HOME/.cargo/config.toml`.

### macOS

On macOS, use the [zld](https://github.com/michaeleisel/zld) linker and keep debuginfo in a single separate file.

Pre-requisites:
- Install [zld](https://github.com/michaeleisel/zld): `brew install michaeleisel/zld/zld`.

`config.toml` (x64):
```toml
[target.x86_64-apple-darwin]
rustflags = [
    "-C",
    "link-arg=-fuse-ld=/usr/local/bin/zld",
    "-C",
    "split-debuginfo=packed",
]
```

`config.toml` (M1):
```toml
[target.aarch64-apple-darwin]
rustflags = [
    "-C",
    "link-arg=-fuse-ld=/opt/homebrew/bin/zld",
    "-C",
    "split-debuginfo=packed",
]
```

### Linux

On Linux, use the [mold](https://github.com/rui314/mold) linker and keep DWARF debuginfo in separate files.

Pre-requisites:
- Install [mold](https://github.com/rui314/mold) through your package manager.

`config.toml`:
```toml
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = [
    "-C",
    "link-arg=-fuse-ld=/usr/bin/mold",
    "-C",
    "split-debuginfo=unpacked",
]
```

### Windows

On Windows, use LLVM's `lld` linker and keep debuginfo in a single separate file.

Pre-requisites:
- Install `lld`:
```
cargo install -f cargo-binutils
rustup component add llvm-tools-preview
```

`config.toml`:
```toml
[target.x86_64-pc-windows-msvc]
linker = "rust-lld.exe"
rustflags = [
    "-C",
    "split-debuginfo=packed",
]
```
