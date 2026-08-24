<h3 align="center">
  <a href="https://www.rerun.io/">
    <img width="1000" height="200" alt="Rerun" src="https://static.rerun.io/d0f5443d4803cac65c73fcc064936c09f5e7f208_rerun_banner.png" />
  </a>
</h3>

# The data layer for physical AI

The data primitives to build, understand, and improve your data loop.
Designed for multi-rate, multimodal data, from the first recording to massive scale.

The open source Rerun Python SDK provides a single toolchain to log, transform, query, view, and train on multi-rate, multimodal data.

## Install

```sh
pip install rerun-sdk
```

The Python module is called `rerun`, while the package published on PyPI is `rerun-sdk`.

See [Install Rerun](https://rerun.io/docs/getting-started/install-rerun) for the Viewer and other SDK languages.

We also provide a [Jupyter widget](https://pypi.org/project/rerun-notebook/) for interactive data visualization in Jupyter notebooks:
```sh
pip install rerun-sdk[notebook]
```

## Example

```py
import numpy as np
import rerun as rr

rr.init("rerun_example_app", spawn=True)

positions = np.vstack([xyz.ravel() for xyz in np.mgrid[3 * [slice(-5, 5, 10j)]]]).T
colors = np.vstack([rgb.ravel() for rgb in np.mgrid[3 * [slice(0, 255, 10j)]]]).astype(np.uint8).T

rr.log("points3d", rr.Points3D(positions, colors=colors))
```

<picture>
  <img src="https://static.rerun.io/pointcloud/80cc95c74b1fbab26af9b0d3547387352f932914/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/pointcloud/80cc95c74b1fbab26af9b0d3547387352f932914/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/pointcloud/80cc95c74b1fbab26af9b0d3547387352f932914/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/pointcloud/80cc95c74b1fbab26af9b0d3547387352f932914/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/pointcloud/80cc95c74b1fbab26af9b0d3547387352f932914/1200w.png">
</picture>

## Resources

- [Examples](https://rerun.io/examples)
- [Python API reference](https://ref.rerun.io/docs/python)
- [Log and ingest data](https://rerun.io/docs/getting-started/data-in)
- [Query and transform data](https://rerun.io/docs/getting-started/data-out)
- [Troubleshooting](https://rerun.io/docs/getting-started/install-rerun/troubleshooting)
- [Discord](https://discord.com/invite/Gcm8BbTaAj)

## Logging and viewing in different processes

The Viewer and Python logger can run in separate processes.
Start the Viewer in one terminal:

```sh
python3 -m rerun
```

In a second terminal, run the example with the `--connect` option:
```sh
python3 examples/python/plots/plots.py --connect
```

Note that SDK and Viewer can run on different machines!
See [SDK operating modes](https://rerun.io/docs/reference/sdk/operating-modes) for connection options.

# Building Rerun from source

Rerun uses [`pixi`](https://pixi.sh/) for development tools and tasks.
[Install pixi](https://pixi.sh/latest/#installation), clone the repository, and run these commands from its `rerun/` directory.

Build and install a development version of the Python SDK:
```sh
pixi run py-build
```

For an optimized build, use:
```sh
pixi run py-build-release
```

Run an example in the development environment:
```sh
pixi run uvpy examples/python/minimal/minimal.py
```

Build a wheel for manual installation:
```sh
pixi run py-build-wheel
```

See [BUILD.md](../BUILD.md) for all Viewer and SDK build options.

# Installing a pre-release

Development wheels built from the latest `main` branch are available from the [prerelease](https://github.com/rerun-io/rerun/releases/tag/prerelease) release.
The `main` branch can be unstable, so use these wheels at your own risk.

# Running Python unit tests

Run the full Python test suite:

```sh
pixi run py-test
```

Build the SDK and run one test file:

```sh
pixi run py-build && pixi run uvpy -m pytest rerun_py/tests/unit/test_tensor.py
```

# Profiling the Python SDK

Install [`puffin_viewer`](https://github.com/EmbarkStudios/puffin), then set `RERUN_PUFFIN=1` when you start a Python program:

```sh
cargo install puffin_viewer
RERUN_PUFFIN=1 pixi run uvpy your_script.py
```

Save a recording from the viewer for offline analysis (use the `investigate-puffin` skill in `.claude/skills/`).
