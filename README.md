<h1 align="center">
  <a href="https://www.rerun.io/">
    <img alt="banner" src="https://user-images.githubusercontent.com/1148717/218142418-1d320929-6b7a-486e-8277-fbeef2432529.png">
  </a>
</h1>

<h1 align="center">
  <a href="https://pypi.org/project/depthai-viewer/">                          <img alt="PyPi"           src="https://img.shields.io/pypi/v/depthai-viewer.svg">                              </a>
  <a href="https://crates.io/crates/rerun">                               <img alt="crates.io"      src="https://img.shields.io/crates/v/rerun.svg">                                </a>
  <a href="https://github.com/rerun-io/rerun/blob/master/LICENSE-MIT">    <img alt="MIT"            src="https://img.shields.io/badge/license-MIT-blue.svg">                        </a>
  <a href="https://github.com/rerun-io/rerun/blob/master/LICENSE-APACHE"> <img alt="Apache"         src="https://img.shields.io/badge/license-Apache-blue.svg">                     </a>
  <a href="https://discord.gg/Gcm8BbTaAj">                                <img alt="Rerun Discord"  src="https://img.shields.io/discord/1062300748202921994?label=Rerun%20Discord"> </a>
</h1>

# Rerun: Visualization infrastructure for computer vision.

Use one of our logging APIs (Python or Rust) to log rich data, such as images and point clouds, to the Rerun Viewer, where it is visualized live or after the fact.

```py
import rerun as rr

viewer.init("my_app", spawn = True) # Spawn a Rerun Viewer and stream log events to it

viewer.log_image("rgb_image", image)
viewer.log_points("points", positions)
viewer.log_rect("car", bbox)
…
```

<p align="center">
  <img width="800" alt="Rerun Viewer" src="https://user-images.githubusercontent.com/1148717/218763490-f6261ecd-e19e-4520-9b25-446ce1ee6328.png">
</p>

## Getting started

- **Python**: `pip install depthai-viewer`
- **Rust**: `cargo add rerun`
- **C / C++**: Coming soon

### Rerun Viewer binary

Both the Python and Rust library can start the Rerun Viewer, but to stream log data over the network or load our `.rrd` data files you also need the `rerun` binary.

It can be installed with `pip install depthai-viewer` or with `cargo install rerun`.

You should now be able to run `rerun --help` in any terminal.

### Documentation

- 📚 [High-level docs](http://rerun.io/docs)
- ⚙️ [Examples](examples)
- 🐍 [Python API docs](https://ref.rerun.io/docs/python)
- 🦀 [Rust API docs](https://docs.rs/rerun/)
- ⁉️ [Troubleshooting](https://www.rerun.io/docs/getting-started/troubleshooting)

## Status

We are in early beta.
There are many features we want to add, and the API is still evolving.
_Expect breaking changes!_

Some shortcomings:

- Big points clouds (1M+) are slow ([#1136](https://github.com/rerun-io/rerun/issues/1136))
- The data you want to visualize must fit in RAM.
  - See <https://www.rerun.io/docs/howto/limit-ram> for how to bound memory use
  - We plan on having a disk-based data store some time in the future
- The Rust library takes a long time to compile
  - We have way too many big dependencies, and we are planning on improving the situation ([#1316](https://github.com/rerun-io/rerun/pull/1316))

## Business model

Rerun uses an open-core model. Everything in this repository will stay open source and free (both as in beer and as in freedom).
In the future, Rerun will offer a commercial product that builds on top of the core free project.

The Rerun open source project targets the needs of individual developers.
The commercial product targets the needs specific to teams that build and run computer vision and robotics products.

# Development

- [`ARCHITECTURE.md`](ARCHITECTURE.md)
- [`BUILD.md`](BUILD.md)
- [`rerun_py/README.md`](rerun_py/README.md) - build instructions for Python SDK
- [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)
- [`CODE_STYLE.md`](CODE_STYLE.md)
- [`CONTRIBUTING.md`](CONTRIBUTING.md)
- [`RELEASES.md`](RELEASES.md)

## Installing a pre-release Python SDK

1. Download the correct `.whl` from [GitHub Releases](https://github.com/rerun-io/rerun/releases)
2. Run `pip install rerun_sdk<...>.whl` (replace `<...>` with the actual filename)
3. Test it: `rerun --version`
