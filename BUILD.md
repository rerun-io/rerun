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

Now install the `pixi` package manager: <https://github.com/prefix-dev/pixi?tab=readme-ov-file#installation>

Make sure `cargo --version` prints `1.74.0` once you are done.

If you are using an Apple-silicon Mac (M1, M2), make sure `rustc -vV` outputs `host: aarch64-apple-darwin`. If not, this should fix it:

```sh
rustup set default-host aarch64-apple-darwin && rustup install 1.74.0
```

Additionally, we use [Cargo-Cranky](https://github.com/ericseppanen/cargo-cranky) for defining which Clippy lints are active and [Cargo-Deny](https://github.com/EmbarkStudios/cargo-deny) for linting crate versions.
You don't need to install these for building, but it's highly recommended when contributing changes to
Rust code.
```sh
cargo install cargo-cranky
cargo install --locked cargo-deny
```

## Building and running the viewer

Use this command for building and running the viewer:

```sh
pixi run rerun
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

Linux/Mac:
```sh
just py-dev-env
source venv/bin/activate
```
Windows (powershell):
```ps1
just py-dev-env
.\venv\Scripts\Activate.ps1
```


Then, the SDK can be compiled and installed in the virtual environment using the following command:

```sh
just py-build
```

This needs to be repeated each time the Rust source code is updated, for example after updating your clone using `git pull`.

Now you can run the python examples from the repository, given that you're still in the virtual environment.
```sh
python examples/python/car/main.py
```

## Building and installing the Rerun C++ SDK

On Windows you have to have a system install of Visual Studio 2022 in order to compile the SDK and samples.

All other dependencies are downloaded by Pixi! You can run tests with:
```sh
just cpp-test
```
and build all C++ artifacts with:
```sh
just cpp-build-all
```

## Building the docs

High-level documentation for rerun can be found at [http://rerun.io/docs](http://rerun.io/docs). It is built from the separate repository [rerun-docs](https://github.com/rerun-io/rerun-docs).

- 🌊 [C++ API docs](https://ref.rerun.io/docs/cpp) are built with `doxygen` and hosted on GitHub. Use `pixi run cpp-docs` to build them locally. For details on the C++ doc-system, see [Writing Docs](https://github.com/rerun-io/rerun/blob/main/rerun_cpp/docs/writing_docs.md).
- 🐍 [Python API docs](https://ref.rerun.io/docs/python) are built via `mkdocs` and hosted on GitHub. For details on the python doc-system, see [Writing Docs](https://github.com/rerun-io/rerun/blob/main/rerun_py/docs/writing_docs.md).
- 🦀 [Rust API docs](https://docs.rs/rerun/) are hosted on  <https://docs.rs/rerun/>. You can build them locally with: `cargo doc --all-features --no-deps --open`.

## Building for the Web

If you want to build a standalone rerun executable that contains the web-viewer and a websocket server,
you need to install the `wasm32-unknown-unknown` rust target and ensure the `web_viewer` feature flag is set when building rerun.
This is automatically done by this shortcut which builds & runs the web viewer:
```
pixi run rerun-web
```

If you're on Windows you have to make sure that your git client creates symlinks,
otherwise you may get errors during the build.
Run `git config --show-scope --show-origin core.symlinks` to check if symlinks are enabled.
You may need to turn on Windows developer mode in order to give the `mklink` command sufficient permissions.
See also this [Stack Overflow reply](https://stackoverflow.com/questions/5917249/git-symbolic-links-in-windows/59761201#59761201) on the issue.


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
