# The Rerun Python Log SDK

Use the Rerun SDK to log data like images, tensors, point clouds, and text. Logs are streamed to the Rerun Viewer for live visualization or to file for later use.

<p align="center">
  <img width="800" alt="Rerun Viewer" src="https://github.com/rerun-io/rerun/assets/2624717/c4900538-fc3a-43b8-841a-8d226e7b5a2e">
</p>

## Install

```sh
pip3 install rerun-sdk
```

ℹ️ Note:
The Python module is called `rerun`, while the package published on PyPI is `rerun-sdk`.

## Example
```py
import rerun as rr
import numpy as np

rr.init("rerun_example_app", spawn=True)

positions = np.vstack([xyz.ravel() for xyz in np.mgrid[3 * [slice(-5, 5, 10j)]]]).T
colors = np.vstack([rgb.ravel() for rgb in np.mgrid[3 * [slice(0, 255, 10j)]]]).astype(np.uint8).T

rr.log("points3d", rr.Points3D(positions, colors=colors))
```

## Resources
* [Examples](https://www.rerun.io/examples)
* [Python API docs](https://ref.rerun.io/docs/python)
* [Quick start](https://www.rerun.io/docs/getting-started/python)
* [Tutorial](https://www.rerun.io/docs/getting-started/logging-python)
* [Troubleshooting](https://www.rerun.io/docs/getting-started/troubleshooting)
* [Discord Server](https://discord.com/invite/Gcm8BbTaAj)

## Logging and viewing in different processes

You can run the viewer and logger in different processes.

In one terminal, start up a viewer with a server that the SDK can connect to:
```sh
python3 -m rerun
```

In a second terminal, run the example with the `--connect` option:
```sh
python3 examples/python/car/main.py --connect
```

-------------------------

# From Source

Setup:

* Install the Rust toolchain: <https://rustup.rs/>
* `git clone git@github.com:rerun-io/rerun.git && cd rerun`
* Run `./scripts/setup_dev.sh`.
* Make sure `cargo --version` prints `1.74.0` once you are done

## Building
To build from source and install Rerun into your *current* Python environment run:

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
# Build the SDK and install in develop mode into the virtualenv
# Re-run this if the Rust code has changed!
source venv/bin/activate
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

## Viewing the docs locally
The rerun python docs are generated using `mkdocs`

Install the doc requirements:
```
pip install -r rerun_py/requirements-doc.txt
```

Serve the docs:
```sh
mkdocs serve -f rerun_py/mkdocs.yml -w rerun_py
```
or
```sh
just py-docs-serve
```

For information on how the docs system works, see: [docs/writing_docs.md](docs/writing_docs.md)


## Troubleshooting
You can run with `RUST_LOG=debug` to get more output out of the rerun SDK.

If you are using an Apple-silicon Mac, make sure `rustc -vV` outputs `host: aarch64-apple-darwin`. If not, this should fix it:

``` sh
rustup set default-host aarch64-apple-darwin && rustup install 1.74
```

If you want to switch back, this is how:
``` sh
rustup set default-host x86_64-apple-darwin && rustup install 1.74
```
