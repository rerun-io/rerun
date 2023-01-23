# The Rerun Python Log SDK.

Goal: an ergonomic Python library for logging rich data, over TCP, to a rerun server.

ℹ️ Note:
- The rust crate is called `re_sdk_python`, while the Python library is called `rerun`.
- These instructions assume you're running from the `rerun` root folder and have Python 3.7 or later available.

## Simply build and install
To build from source and install the `rerun` into your *current* Python environment run:

```sh
python3 -m pip install --upgrade pip
pip3 install "./rerun_py"
```

ℹ️ Note:
- If you are unable to upgrade pip to version `>=21.3`, you need to pass `--use-feature=in-tree-build` to the `pip3 install` command.

## Usage
See [`USAGE.md`](USAGE.md).

## Running the example code
```sh
python examples/car/main.py
```

By default, the example runs Rerun in buffered mode, in the same process as the example code. This means all logged data is buffered until `rerun.show()` is called in the end, which shows the viewer and blocks until the viewer is closed.

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
# Build the SDK and install in develop mode into the virtualenv
# Re-run this if the Rust code has changed!
just py-build
```

### Test
```sh
# Run the unit tests
just py-test

# Run the linting checks
just py-lint

# Run an example
python examples/car/main.py
```

### Logging and viewing in different processes

Rerun can also be run in non-blocking mode with viewer and logger in different processes.

In one terminal, start up a viewer with a server that the SDK can connect to:
```sh
cargo run -p rerun --release
```

In a second terminal, run the example with the `--connect` option:
```sh
examples/car/main.py --connect
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

```
pip3 install target/wheels/*.whl
```

## Viewing the docs locally
The rerun python docs are generated using `mkdocs`

Install the doc requirements:
```
pip install -r rerun_py/requirements-doc.txt
```

Serve the docs:
```
cd rerun_py && mkdocs serve
```


## Troubleshooting
You can run with `RUST_LOG=debug` to get more output out of the rerun SDK.

If you are using an Apple-silicon Mac, make sure `rustc -vV` outputs `host: aarch64-apple-darwin`. If not, this should fix it:

``` sh
rustup set default-host aarch64-apple-darwin && rustup install 1.65
```

If you want to switch back, this is how:
``` sh
rustup set default-host x86_64-apple-darwin && rustup install 1.65
```
