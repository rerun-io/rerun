# Rerun
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-MIT)
[![Apache](https://img.shields.io/badge/license-Apache-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-APACHE)

Rerun is visualization infrastructure for computer vision.

This repository contains the Rerun SDK and Rerun Viewer. Use the SDK (currently Python only) to log rich data that is streamed to the viewer, where it is visualized live or after the fact.

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

## Build and install the Rerun Python SDK
### Set up virtualenv
```sh
python3 -m venv venv  # Rerun supports Python version >= 3.7
source venv/bin/activate
python -m pip install --upgrade pip  # We need pip version >=21.3
```
From here on out, we assume you have this virtualenv activated.

### Build and install
``` sh
pip install ./rerun_py
```
> Note: If you are unable to upgrade pip to version `>=21.3`, you need to pass `--use-feature=in-tree-build` to the `pip install` command.

## Getting started with examples
The easiest way to get started is to run and look at [`examples`](examples).

### Install dependencies and run an example
Each example comes with its own set of dependencies listed in a `requirements.txt` file. To install dependencies and run the `car` example (a toy example that doesn't need to download data) run:
```sh
pip install -r examples/car/requirements.txt  # Install the dependencies needed to run the car example
python examples/car/main.py
```
You can install dependencies needed run all examples by running:
```sh
pip install -r examples/requirements.txt  # Install the dependencies needed to run all car examples
```
### Buffered or live visualization
By default, the examples run in buffered mode. This means they run through the whole example, and then show the viewer (UI) at the end in the same process by calling blocking function `rerun.show()`.

If you'd rather see the visualizations live, as data is being logged. Run the examples with the `--connect` flag. The Rerun SDK will then try to connect to a Rerun Viewer running in another process and send the data as it is produced.

To visualize an example live, first in one terminal (with the activated virtualenv) run:
```sh
python -m rerun_sdk  # Opens a Rerun Viewer that will wait for data from the Rerun SDK
```
Then run the example in a second terminal like:
```sh
python examples/car/main.py --connect  # The Rerun SDK will connect and send data to the separate viewer.
```

## Using the Rerun Python SDK
Most documentation is found in the docstrings of the functions in the Rerun. Either check out the docstrings directly in code or use the built in `help()` function. For example, to see the docstring of the `log_image` function, open a python terminal and run:
```python
import rerun_sdk as rerun
help(rerun.log_image)
```
For a description of how to use the SDK, including some of the key concepts, see [`rerun_py/USAGE.md`](rerun_py/USAGE.md).

## Rerun Viewer without Python
You can also build and install the Rerun Viewer to be used from the terminal without going through Python.

To build and install run:
```sh
cargo install --path ./crates/rerun/
```
You should now be able to run `rerun --help` in any terminal.

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
