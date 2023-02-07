# Building Rerun
This is a guide to how to build Rerun.


## Getting started with the repository.
* Install the Rust toolchain: <https://rustup.rs/>
* `git clone git@github.com:rerun-io/rerun.git && cd rerun`
* Run `./scripts/setup_dev.sh`.
* Make sure `cargo --version` prints `1.67.0` once you are done


### Apple-silicon Macs

If you are using an Apple-silicon Mac (M1, M2), make sure `rustc -vV` outputs `host: aarch64-apple-darwin`. If not, this should fix it:

```sh
rustup set default-host aarch64-apple-darwin && rustup install 1.67
```

## Build and install the Rerun Python SDK

### Set up virtualenv

Mac/Linux:

```sh
python3 -m venv venv  # Rerun supports Python version >= 3.7
source venv/bin/activate
python -m pip install --upgrade pip  # We need pip version >=21.3
```

Windows (powershell):

```ps1
python -m venv venv
.\venv\Scripts\Activate.ps1
python -m pip install --upgrade pip
```

From here on out, we assume you have this virtualenv activated.

### Build and install

```sh
./scripts/setup.sh
pip install ./rerun_py
```

> Note: If you are unable to upgrade pip to version `>=21.3`, you need to pass `--use-feature=in-tree-build` to the `pip install` command.


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
