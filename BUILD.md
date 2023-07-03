# Building Rerun
This is a guide to how to build Rerun.


## See also
* [`rerun_py/README.md`](rerun_py/README.md) - build instructions for Python SDK
* [`ARCHITECTURE.md`](ARCHITECTURE.md)
* [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)
* [`CODE_STYLE.md`](CODE_STYLE.md)
* [`CONTRIBUTING.md`](CONTRIBUTING.md)
* [`RELEASES.md`](RELEASES.md)


## Getting started with the repository

First, install the Rust toolchain using the installer from <https://rustup.rs/>.

Then, clone the repository:
```sh
git clone git@github.com:rerun-io/rerun.git
cd rerun
```

Finally, run the following script to install the dependencies and CLI tools needed for Rerun's build environment:

```sh
./scripts/setup_dev.sh
```

Make sure `cargo --version` prints `1.69.0` once you are done.

If you are using an Apple-silicon Mac (M1, M2), make sure `rustc -vV` outputs `host: aarch64-apple-darwin`. If not, this should fix it:

```sh
rustup set default-host aarch64-apple-darwin && rustup install 1.69.0
```

## Building and running the viewer

Use this command for building and running the viewer:

```sh
cargo rerun
```

This custom cargo command is enabled by an alias located in `.cargo/config.toml`.


## Running the Rust examples

All Rust examples are set up as separate executables, so they can be run by specifying the corresponding package, for example:

```sh
cargo run -p dna
```


## Building and installing the Rerun Python SDK

Rerun is available as a package on PyPi and can be installed with `pip install rerun-sdk`.

Additionally, prebuilt dev wheels from head of main are available at <https://github.com/rerun-io/rerun/releases/tag/prerelease>.

If you want to build from source, use the following instructions.

### Mac/Linux

First, a local virtual environment must be created and the necessary dependencies installed (this needs to be done only once):

```sh
just py-dev-env
source venv/bin/activate
```

Then, the SDK can be compiled and installed in the virtual environment using the following command:

```sh
just py-build
```

This needs to be repeated each time the Rust source code is updated, for example after updating your clone using `git pull`.

Finally, the virtual environment must be activated to run Python examples:

```sh
source venv/bin/activate
python examples/python/car/main.py
```

### Windows (PowerShell)

The `justfile` currently doesn't support Windows, so each step must be run manually.

First, create and activate a local virtual environment and install the required dependencies using the following commands:

```ps1
python -m venv venv
.\venv\Scripts\Activate.ps1
python -m pip install --upgrade pip
python -m pip install -r scripts/requirements-dev.txt
```

Then build and install the Rerun SDK with:

```ps1
maturin develop \
    --manifest-path rerun_py/Cargo.toml \
    --extras="tests"
```

You can then run the example using the following command:

```ps1
python examples/python/car/main.py
```

## Building the docs

High-level documentation for rerun can be found at [http://rerun.io/docs](http://rerun.io/docs). It is built from the separate repository [rerun-docs](https://github.com/rerun-io/rerun-docs).

Python API docs can be found at <https://ref.rerun.io/docs/python> and are built via `mkdocs` and hosted on GitHub. For details on the python doc-system, see [Writing Docs](https://github.com/rerun-io/rerun/blob/main/rerun_py/docs/writing_docs.md).

Rust documentation is hosted on <https://docs.rs/rerun/>. You can build them locally with: `cargo doc --all-features --no-deps --open`


## Building for the Web

If you want to build a standalone rerun executable that contains the web-viewer and a websocket server,
you need to ensure the `web_viewer` feature flag is set:
```
cargo build -p rerun --features web_viewer
```

Rerun uses a standalone tool to build the web-viewer. You can invoke it directly as well:
```
cargo run -p re_build_web_viewer -- --release
```


### Building with WebGPU support

By default, all web builds are using WebGL for rendering.
However, Rerun can also build with experimental WebGPU support!
Note that currently we can't build wasm files that support both WebGPU and WebGL.

To build a standalone Rerun executable with a WebGPU web viewer, you need to set
the `RERUN_BUILD_WEBGPU` env variable and enable the  `web_viewer` feature:
```
RERUN_BUILD_WEBGPU=1 cargo build -p rerun --features web_viewer
```

And for building a WebGPU based web-viewer without the server:
```
cargo run -p re_build_web_viewer -- --release --webgpu
```

## Improving compile times

As of today, we link everything statically in both debug and release builds, which makes custom linkers and split debuginfo the two most impactful tools we have at our disposal in order to improve compile times.

These tools can be configured through your `Cargo` configuration, available at `$HOME/.cargo/config.toml`.

### macOS

On x64 macOS, use the [zld](https://github.com/michaeleisel/zld) linker and keep debuginfo in a single separate file.

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

On Apple-silicon Mac (M1, M2), the default settings are already pretty good. The default linker is just as good as `zld`. Do NOT set `split-debuginfo=packed`, as that will make linking a lot slower. You can set `split-debuginfo=unpacked` for a small improvement.

`config.toml` (M1, M2):
```toml
[target.aarch64-apple-darwin]
rustflags = [
    "-C",
    "split-debuginfo=unpacked",
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
