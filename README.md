# Rerun

[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-MIT)
[![Apache](https://img.shields.io/badge/license-Apache-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-APACHE)

Rerun is visualization infrastructure for computer vision.

This repository contains the Rerun SDK and Rerun Viewer. Use the SDK (currently Python only) to log rich data that is streamed to the viewer, where it is visualized live or after the fact.

# For our users

We don't have any pre-built binaries yet, so you need to build Rerun from source. There is some setup involved, but most of it should be pretty painless.

## Setup

- Install the Rust toolchain: <https://rustup.rs/>
- `git clone git@github.com:rerun-io/rerun.git && cd rerun`
- Run `./scripts/setup.sh`.
- Make sure `cargo --version` prints `1.67.0` once you are done

### Apple-silicon Macs

If you are using an Apple-silicon Mac, make sure `rustc -vV` outputs `host: aarch64-apple-darwin`. If not, this should fix it:

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

## Documentation

Our documentation lives in Markdown files inside `/docs`. Contributions are welcome. Changes frequently go live on our website, but not automatically.

### Special syntax

Code examples can be rendered in multiple languages by placing them in `docs/code-examples`, like so:

```
/docs
    /code-examples
        /my-example
            /example.py
            /example.rs
```

The can then be referenced in Markdown using this syntax:

```
code-example: my-example
```

# Development

Take a look at [`CONTRIBUTING.md`](CONTRIBUTING.md).
