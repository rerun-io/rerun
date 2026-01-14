# The Rerun Python SDK

Use the Rerun SDK to record data like images, tensors, point clouds, and text. Data is streamed to the Rerun Viewer for live visualization or to file for later use.

<p align="center">
  <img width="800" alt="Rerun Viewer" src="https://github.com/rerun-io/rerun/assets/2624717/c4900538-fc3a-43b8-841a-8d226e7b5a2e">
</p>

## Install

```sh
pip3 install rerun-sdk
```

ℹ️ Note:
The Python module is called `rerun`, while the package published on PyPI is `rerun-sdk`.

For other SDK languages see [Installing Rerun](https://www.rerun.io/docs/getting-started/installing-viewer).

We also provide a [Jupyter widget](https://pypi.org/project/rerun-notebook/) for interactive data visualization in Jupyter notebooks:
```sh
pip3 install rerun-sdk[notebook]
```

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
* [Quick start](https://www.rerun.io/docs/getting-started/quick-start/python)
* [Tutorial](https://www.rerun.io/docs/getting-started/data-in/python)
* [Troubleshooting](https://www.rerun.io/docs/getting-started/troubleshooting)
* [Discord Server](https://discord.com/invite/Gcm8BbTaAj)

## Logging and viewing in different processes

You can run the Viewer and logger in different processes.

In one terminal, start up a Viewer with a server that the SDK can connect to:
```sh
python3 -m rerun
```

In a second terminal, run the example with the `--connect` option:
```sh
python3 examples/python/plots/plots.py --connect
```
Note that SDK and Viewer can run on different machines!


# Building Rerun from source

We use [`pixi`](https://pixi.sh/) for managing dev-tool versioning, download and task running. See [here](https://pixi.sh/latest/#installation) for installation instructions.

```sh
pixi run py-build
```
This builds the SDK for Python (use `pixi run py-build --release` for a release build).

You can then run examples via uv:
```sh
pixi run uv run examples/python/minimal/minimal.py
```

To build a wheel instead for manual install use:
```sh
pixi run py-build-wheel
```

Refer to [BUILD.md](../BUILD.md) for details on the various different build options of the Rerun Viewer and SDKs for all target languages.


# Installing a pre-release

Prebuilt dev wheels from head of main are available at <https://github.com/rerun-io/rerun/releases/tag/prerelease>.

While we try to keep the main branch usable at all times, it may be unstable occasionally. Use at your own risk.


# Running Python unit tests
```sh
pixi run py-test
```

If you run into a problem, run `rm -rf .pixi .venv` and try again.

# Running specific Python unit tests
```sh
pixi run py-build && pixi run uvpy -m pytest rerun_py/tests/unit/test_tensor.py
```
