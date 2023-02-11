<h1 align="center">
  <img alt="banner" src="https://user-images.githubusercontent.com/1148717/218142418-1d320929-6b7a-486e-8277-fbeef2432529.png">
</h1>

[![CI (Python)](https://github.com/rerun-io/rerun/actions/workflows/python.yml/badge.svg?branch=main)](https://github.com/rerun-io/rerun/actions/workflows/python.yml)
[![CI (Rust)](https://github.com/rerun-io/rerun/actions/workflows/rust.yml/badge.svg?branch=main)](https://github.com/rerun-io/rerun/actions/workflows/rust.yml)
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-MIT)
[![Apache](https://img.shields.io/badge/license-Apache-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-APACHE)
[![Discord](https://img.shields.io/discord/900275882684477440?label=Rerun%20Community%20Discord)](https://discord.gg/Gcm8BbTaAj)

# Rerun: Visualization infrastructure for computer vision.

Use one of our logging APIs (Python or Rust) to log rich data, such as images and point clouds, to the Rerun Viewer, where it is visualized live or after the fact.

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

# Setup

## Python

Install the latest Rerun SDK version with:

```sh
pip install rerun-sdk
```

## Rust
Coming soon
<!-- TODO(#1161): `cargo add rerun` + `cargo install rerun` -->

## C

Coming soon


# Documentation
- [Examples](examples)
- [Python API docs](https://rerun-io.github.io/rerun)
- [Rust getting-started guide](rerun_py/USAGE.md)
<!--- TODO(#1161): update doclinks
- [High-level documentation](http://rerun.io/docs)
- [Rust API docs](https://docs.rs/rerun/)
-->


## Rerun Viewer without Python
You can also build and install the Rerun Viewer to be used from the terminal without going through Python.

To build and install run:

```sh
cargo install --path ./crates/rerun/
```

You should now be able to run `rerun --help` in any terminal.


## Shortcomings
* Big points clouds (1M+) are slow ([#1136](https://github.com/rerun-io/rerun/issues/1136))
* The data you want to visualize must fit in RAM.
  - See [`rerun_py/USAGE.md`](rerun_py/USAGE.md) for how to bound memory use
  - We plan on having a disk-based data store some time in the future
  - Additionally, Rerun is using more memory than it should at the moment


## Business model
Rerun uses an open core model. Everything in this repository will stay open source and free (as in beer), forever. In the future, Rerun will offer a commercial product that builds on top of the core free project.

The Rerun open source project targets the needs of individual developers. The commercial product targets the needs specific to teams that build and run computer vision and robotics products.


# Development
* [`ARCHITECTURE.md`](ARCHITECTURE.md)
* [`BUILD.md`](BUILD.md)
* [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)
* [`CODE_STYLE.md`](CODE_STYLE.md)
* [`CONTRIBUTING.md`](CONTRIBUTING.md)
* [`RELEASES.md`](RELEASES.md)


## Installing a pre-release Python SDK

1. Download the correct `.whl` from [GitHub Releases](https://github.com/rerun-io/rerun/releases)
2. Run `pip install rerun_sdk<...>.whl` (replace `<...>` with the actual filename)
3. Test it: `rerun --version`
