# The Rerun Python Log SDK

Rerun is an SDK for logging computer vision and robotics data paired with a visualizer for exploring that data over time.
It lets you debug and understand the internal state and data of your systems with minimal code.

<p align="center">
  <img width="800" alt="Rerun Viewer" src="https://user-images.githubusercontent.com/1148717/218763490-f6261ecd-e19e-4520-9b25-446ce1ee6328.png">
</p>

## Install

```sh
pip3 install depthai-viewer
```

ℹ️ Note:
The Python module is called `rerun`, while the package published on PyPI is `depthai-viewer`.

## Example

```py
import rerun as rr
import numpy as np

viewer.spawn()

positions = np.vstack([xyz.ravel() for xyz in np.mgrid[3 * [slice(-5, 5, 10j)]]]).T
colors = np.vstack([rgb.ravel() for rgb in np.mgrid[3 * [slice(0, 255, 10j)]]]).astype(np.uint8).T

viewer.log_points("my_points", positions=positions, colors=colors)
```

## Resources

- [Quick start](https://www.rerun.io/docs/getting-started/python)
- [Python API docs](https://ref.rerun.io/docs/python)
- [Tutorial](https://www.rerun.io/docs/getting-started/logging-python)
- [Examples on GitHub](https://github.com/rerun-io/rerun/tree/latest/examples/python)
- [Troubleshooting](https://www.rerun.io/docs/getting-started/troubleshooting)
- [Discord Server](https://discord.com/invite/Gcm8BbTaAj)

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

---

# From Source

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

For information on how the docs system works, see: [docs/docs.md](docs/docs.md)

## Troubleshooting

You can run with `RUST_LOG=debug` to get more output out of the rerun SDK.

If you are using an Apple-silicon Mac, make sure `rustc -vV` outputs `host: aarch64-apple-darwin`. If not, this should fix it:

```sh
rustup set default-host aarch64-apple-darwin && rustup install 1.67
```

If you want to switch back, this is how:

```sh
rustup set default-host x86_64-apple-darwin && rustup install 1.67
```
