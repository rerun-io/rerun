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

Make sure `cargo --version` prints `1.81.0` once you are done.

If you are using an Apple-silicon Mac (M1, M2), make sure `rustc -vV` outputs `host: aarch64-apple-darwin`. If not, this should fix it:

```sh
rustup set default-host aarch64-apple-darwin && rustup install 1.81.0
```

## Git-lfs

We use [git-lfs](https://git-lfs.com/) to store big files in the repository, such as UI test snapshots.
We aim to keep this project buildable without the need of git-lfs (for example, icons and similar assets are checked in to the repo as regular files).
However, git-lfs is generally required for a proper development environment, e.g. to run tests.

### Setting up git-lfs

The TL;DR is to install git-lfs via your favorite package manager (`apt`, Homebrew, MacPorts, etc.) and run `git lfs install`.
See the many resources available online more details.

You can ensure that everything is correctly installed by running `git lfs ls-files` from the repository root.
It should list some test snapshot files.


## Validating your environment
You can validate your environment is set up correctly by running:
```sh
pixi run check-env
```


## Building and running the Viewer

Use this command for building and running the viewer:

```sh
pixi run rerun
```


## Running the Rust examples

All Rust examples are set up as separate executables, so they can be run by specifying the corresponding package, for example:

```sh
cargo run -p dna
```

They will either connect to an already running rerun viewer, or spawn a new one.
In debug builds, it will spawn `target/debug/rerun` if it exists, otherwise look for `rerun` on `PATH`.


## Building and installing the Rerun Python SDK

Rerun is available as a package on PyPi and can be installed with `pip install rerun-sdk`.

Additionally, nightly dev wheels from head of `main` are available at <https://github.com/rerun-io/rerun/releases/tag/prerelease>.

If you want to build from source, you can do so easily in the Pixi environment:
* Run `pixi run py-build --release` to build SDK & Viewer for Python (or `pixi run py-build` for a debug build)
* Then you can run examples from the repository, either by making the Pixi shell active with  `pixi shell` and then running Python or by using `pixi run`, e.g. `pixi run Python examples/python/minimal/minimal.py`


### Tests & tooling

```sh
# Run the unit tests
pixi run py-test

# Run the linting checks
pixi run py-lint

# Run the formatter
pixi run py-fmt
```

### Building an installable Python wheel
The `py-build-wheels-sdk-only` command builds a whl file:
```sh
pixi run py-build-wheels-sdk-only
```
Which you can then install in your own Python environment:
```sh
pip install ./dist/CURRENT_ARCHITECTURE/*.whl
```

**IMPORTANT**: unlike the official wheels, wheels produced by this method do _not_ contain the viewer, so they may only be used for logging purposes.

## Building and installing the Rerun C++ SDK

On Windows you have to have a system install of Visual Studio 2022 in order to compile the SDK and samples.

All other dependencies are downloaded by Pixi! You can run tests with:
```sh
pixi run -e cpp cpp-test
```
and build all C++ artifacts with:
```sh
pixi run -e cpp cpp-build-all
```

## Building the docs

High-level documentation for Rerun can be found at [http://rerun.io/docs](http://rerun.io/docs). It is built from the separate repository [rerun-docs](https://github.com/rerun-io/rerun-docs).

- 🌊 [C++ API docs](https://ref.rerun.io/docs/cpp) are built with `doxygen` and hosted on GitHub. Use `pixi run -e cpp cpp-docs` to build them locally. For details on the C++ doc-system, see [Writing Docs](rerun_cpp/docs/writing_docs.md).
- 🐍 [Python API docs](https://ref.rerun.io/docs/python) are built via `mkdocs` and hosted on GitHub. For details on the Python doc-system, see [Writing Docs](rerun_py/docs/writing_docs.md).
- 🦀 [Rust API docs](https://docs.rs/rerun/) are hosted on  <https://docs.rs/rerun/>. You can build them locally with: `cargo doc --all-features --no-deps --open`.

## Building for the web

If you want to build a standalone Rerun executable that contains the web-viewer and a websocket server,
you need to install the `wasm32-unknown-unknown` Rust target and ensure the `web_viewer` feature flag is set when building rerun.
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
