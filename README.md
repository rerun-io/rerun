# Rerun

[![Build Status](https://github.com/emilk/egui/workflows/CI/badge.svg)](https://github.com/emilk/egui/actions?workflow=CI)
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-MIT)
[![Apache](https://img.shields.io/badge/license-Apache-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-APACHE)
[![Discord](https://img.shields.io/discord/900275882684477440?label=Rerun%20Community%20Discord)](https://discord.gg/Gcm8BbTaAj)


Rerun is visualization infrastructure for computer vision.

This repository contains the Rerun SDK and Rerun Viewer. Use the SDK (currently Python only) to log rich data that is streamed to the viewer, where it is visualized live or after the fact.

# Documentation (Coming soon!)

## WARNING: The following links don't all work yet
<!-- TODO(jleibs): Clean up this section and remove warning when all links are live -->

High-level documentation for rerun can be found at [http://rerun.io/docs](http://rerun.io/docs).

The documentation is built from a separate [Rerun-Docs Repository](https://github.com/rerun-io/rerun-docs).

Rust and Python APIs are documented in the code via docstrings.
 - The [Rust API docs](https://docs.rs/rerun/) are built via cargo and hosted on docs.rs
 - The [Python API docs](https://rerun-io.github.io/rerun) are built via mkdocs and hosted on github:
   - For more information on the python doc-system see: [Writing Docs](https://rerun-io.github.io/rerun/latest/docs)

# Installing the Python Rerun SDK
1. Download the correct `.whl` from [GitHub Releases](https://github.com/rerun-io/rerun/releases)
  (for Mac M1/M2, grab the "universal2" `.whl`)
2. Uninstall any previously installed Rerun SDK: `pip uninstall rerun rerun-sdk`
3. Run `pip install rerun_sdk<...>.whl` (replace `<...>` with the actual filename)
4. Test it: `rerun --version`

# Installing the Rerun SDK
Coming soon

## Getting started with examples

The easiest way to get started is to run and look at [`examples`](examples).

### Buffered or live visualization

By default, the examples run in buffered mode. This means they run through the whole example, and then show the viewer (UI) at the end in the same process by calling blocking function `rerun.show()`.

If you'd rather see the visualizations live, as data is being logged. Run the examples with the `--connect` flag. The Rerun SDK will then try to connect to a Rerun Viewer running in another process and send the data as it is produced.

To visualize an example live, first in one terminal (with the activated virtualenv) run:

```sh
python -m rerun  # Opens a Rerun Viewer that will wait for data from the Rerun SDK
```

Then run the example in a second terminal like:

```sh
python examples/car/main.py --connect  # The Rerun SDK will connect and send data to the separate viewer.
```

## Using the Rerun Python SDK

Most documentation is found in the docstrings of the functions in the Rerun. Either check out the docstrings directly in code or use the built in `help()` function. For example, to see the docstring of the `log_image` function, open a python terminal and run:

```python
import rerun as rr
help(rr.log_image)
```

For a description of how to use the SDK, including some of the key concepts, see [`rerun_py/USAGE.md`](rerun_py/USAGE.md).

## Rerun Viewer without Python

You can also build and install the Rerun Viewer to be used from the terminal without going through Python.

To build and install run:

```sh
cargo install --path ./crates/rerun/
```

You should now be able to run `rerun --help` in any terminal.

## Bounded memory use

You can set `--memory-limit=16GB` to tell the Rerun Viewer to purge older log data when memory use goes above that limit. This is useful for using Rerun in _continuous_ mode, i.e. where you keep logging new data to Rerun forever.

It is still possible to log data faster than the Rerun Viewer can process it, and in those cases you may still run out of memory unless you also set `--drop-at-latency=200ms` or similar.


# Development

Take a look at [`BUILD.md`](BUILD.md), [`CONTRIBUTING.md`](CONTRIBUTING.md).
