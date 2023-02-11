# The Rerun Python Log SDK

Log rich data, such as images and point clouds, and instantly visualize them, with time scrubbing.

`pip install rerun-sdk`

```py
import rerun as rr

rr.init("my_app", spawn = True) # Spawn a Rerun Viewer and stream log events to it

rr.log_image("rgb_image", image)
rr.log_points("points", positions)
rr.log_rect("car", bbox)
…
```

<p align="center">
<img src="https://user-images.githubusercontent.com/1148717/218265704-1863c270-1422-48fe-9009-d67f8133c4cc.gif">
</p>

## Getting started
See [`USAGE.md`](USAGE.md).

<!-- TODO(#1161): add links to our docs! -->

## Notes
- The rust crate is called `rerun_py`, the Python module is called `rerun`, and the package published on PyPI is `rerun-sdk`.
- These instructions assume you're running from the `rerun` root folder and have Python 3.8 or later available.

## Building from Source
To build from source and install the `rerun` into your *current* Python environment run:

```sh
python3 -m pip install --upgrade pip
pip3 install "./rerun_py"
```

ℹ️ Note:
- If you are unable to upgrade pip to version `>=21.3`, you need to pass `--use-feature=in-tree-build` to the `pip3 install` command.


## Running the example code
```sh
python examples/python/car/main.py
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
python examples/python/car/main.py
```

### Logging and viewing in different processes

Rerun can also be run in non-blocking mode with viewer and logger in different processes.

In one terminal, start up a viewer with a server that the SDK can connect to:
```sh
cargo run -p rerun --release
```

In a second terminal, run the example with the `--connect` option:
```sh
examples/python/car/main.py --connect
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
mkdocs serve -f rerun_py/mkdocs.yml -w rerun_py
```
or
```
just py-docs-serve
```

For information on how the docs system works, see: [docs/docs.md](docs/docs.md)


## Troubleshooting
You can run with `RUST_LOG=debug` to get more output out of the rerun SDK.

If you are using an Apple-silicon Mac, make sure `rustc -vV` outputs `host: aarch64-apple-darwin`. If not, this should fix it:

``` sh
rustup set default-host aarch64-apple-darwin && rustup install 1.67
```

If you want to switch back, this is how:
``` sh
rustup set default-host x86_64-apple-darwin && rustup install 1.67
```
