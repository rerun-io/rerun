# Depthai Viewer

![Screenshot from 2023-05-20 00-22-36](https://github.com/luxonis/depthai-viewer/assets/59307111/605bdf38-1bb4-416d-9643-0da1a511d58e)

## Install

```sh
python3 -m pip install depthai-viewer
```

## Run

```sh
depthai-viewer
# OR
python3 -m depthai_viewer
```

# Building From Source

Setup:

- Install the Rust toolchain: <https://rustup.rs/>
- `git clone git@github.com:rerun-io/rerun.git && cd rerun`
- Run `./scripts/setup_dev.sh`.
- Make sure `cargo --version` prints `1.67.1` once you are done

## Building

To build from source and install Rerun into your _current_ Python environment run:

```sh
python3 -m pip install --upgrade pip
pip3 install -r rerun_py/requirements-build.txt
pip3 install "./rerun_py"
```

ℹ️ Note:
If you are unable to upgrade pip to version `>=21.3`, you need to pass `--use-feature=in-tree-build` to the `pip3 install` command.

## Development

To set up a new virtualenv for development:

```sh
just py-dev-env
# For bash/zsh users:
source venv/bin/activate
# Or if you're using fish:
source venv/bin/activate.fish
```

## Build, test, and run

For ease of development you can build and install in "editable" mode. This means you can edit the `rerun` Python code without having to re-build and install to see changes.

```sh
# Build the depthai-viewer and install in develop mode into the virtualenv
# Re-run this if the Rust code has changed!
just py-build
```

### Test

```sh
# Run the unit tests
just py-test

# Run the linting checks
just py-lint

```

## Building an installable Python Wheel

The Python bindings to the core Rust library are built using https://github.com/PyO3/pyo3.

To build an installable Python wheel run:

```
pip install -r rerun_py/requirements-build.txt
maturin build -m rerun_py/Cargo.toml --release
```

By default the wheels will be built to `target/wheels` (use the `-o` flag to set a different output directory).

Now you can install `rerun` in any Python3 environment using:

```sh
pip3 install target/wheels/*.whl
```

## Troubleshooting

You can run with `RUST_LOG=debug` to get more output out of the depthai-viewer.

If you are using an Apple-silicon Mac, make sure `rustc -vV` outputs `host: aarch64-apple-darwin`. If not, this should fix it:

```sh
rustup set default-host aarch64-apple-darwin && rustup install 1.67
```

If you want to switch back, this is how:

```sh
rustup set default-host x86_64-apple-darwin && rustup install 1.67
```
