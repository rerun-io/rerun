# The Rerun Python Log SDK.

Goal: an ergonomic Python library for logging rich data, over TCP, to a rerun server.

ℹ️ Note: The rust crate is called `re_sdk_python`, while the Python library is called `rerun_sdk`.

## Build and install
To build and install the `rerun_sdk` into your *current* Python environment run:

```sh
python3 -m pip install --upgrade pip
pip3 install "py-rerun"
```

ℹ️ Notes:
- If you are unable to upgrade pip to version `>=21.3`, you need to pass `--use-feature=in-tree-build` to the `pip3 install` command.
-  `[tests,examples]` here is used to specify that we also want to install the dependencies needed for running tests and examples.

## Usage
See [`USAGE.md`](USAGE.md).

## Running the example code
```sh
python rerun_sdk/examples/example_dummy.py
```

By default, the example runs Rerun in buffered mode, in the same process as the example code. This means all logged data is buffered until `rerun_sdk.show()` is called in the end, which shows the viewer and blocks until the viewer is closed.

## Development

ℹ️ Note:
- These instructions assume you're running from the `py-rerun` folder and have Python 3.7 or later available.
- We make use of the [`just`](https://github.com/casey/just) command runner tool for repository automation. See [here](https://github.com/casey/just#installation) for installation instructions.

## Setup

To set up a new virtualenv for development:

```sh
just develop-venv
# For bash/zsh users:
source venv/bin/activate
# Or if you're using fish:
source venv/bin/activate.fish
```

## Build, test, and run

For ease of development you can build and install in "editable" mode. This means you can edit the `rerun_sdk` Python code without having to re-build and install to see changes.

```sh
# Build the SDK and install in develop mode into the virtualenv
# Re-run this if the Rust code has changed!
just build
```

### Test
```sh
# Run the unit tests
just test

# Run the linting checks
just lint

# Run an example
example_dummy
```

### Logging and viewing in different processes

Rerun can aslo be run in non-blocking mode with viewer and logger in different processes.

In one terminal, start up a viewer with a server that the SDK can connect to:
```sh
cargo run -p rerun --release
```

In a second terminal, run the example with the `--connect` option:
```sh
example_dummy --connect
```

## Building an installable Python Wheel
The Python bindings to the core Rust library are built using https://github.com/PyO3/pyo3.

To build an installable Python wheel run:
```
pip install -r py-rerun/requirements-build.txt
maturin build -m py-rerun/Cargo.toml --release
```

By default the wheels will be built to `target/wheels` (use the `-o` flag to set a different output directory).

Now you can install `rerun_sdk` in any Python3 environment using:

```
pip3 install target/wheels/*.whl
```


## Troubleshooting
You can run with `RUST_LOG=debug` to get more output out of the rerun SDK.

If you are using an Apple-silicon Mac, make sure `rustc -vV` outputs `host: aarch64-apple-darwin`. If not, this should fix it:

``` sh
rustup set default-host aarch64-apple-darwin && rustup install 1.63
```

If you want to switch back, this is how:
``` sh
rustup set default-host x86_64-apple-darwin && rustup install 1.63
```
