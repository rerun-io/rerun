<h1 align="center">
  <a href="https://www.rerun.io/">
    <img alt="banner" src="https://user-images.githubusercontent.com/1148717/218142418-1d320929-6b7a-486e-8277-fbeef2432529.png">
  </a>
</h1>

[![CI (Python)](https://github.com/rerun-io/rerun/actions/workflows/python.yml/badge.svg?branch=main)](https://github.com/rerun-io/rerun/actions/workflows/python.yml)
[![CI (Rust)](https://github.com/rerun-io/rerun/actions/workflows/rust.yml/badge.svg?branch=main)](https://github.com/rerun-io/rerun/actions/workflows/rust.yml)
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-MIT)
[![Apache](https://img.shields.io/badge/license-Apache-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-APACHE)
[![Discord](https://img.shields.io/discord/1062300748202921994?label=Rerun%20Discord)](https://discord.gg/Gcm8BbTaAj)

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
  <img width="800" alt="Rerun Viewer" src="https://user-images.githubusercontent.com/1148717/218763490-f6261ecd-e19e-4520-9b25-446ce1ee6328.png">
</p>

## Getting started
* **Python**: `pip install rerun-sdk`
* **Rust**: `cargo add rerun`
* **C / C++**: Coming soon

### Rerun Viewer binary
Both the Python and Rust library can start the Rerun Viewer, but to stream log data over the network or load our `.rrd` data files you also need the `rerun` binary.

It can be installed with `pip install rerun-sdk` or with `cargo install rerun`.

You should now be able to run `rerun --help` in any terminal.


### Documentation
- 📚 [High-level docs](http://rerun.io/docs)
- ⚙️ [Examples](examples)
- 🐍 [Python API docs](https://rerun-io.github.io/rerun)
- 🦀 [Rust API docs](https://docs.rs/rerun/)



## Shortcomings
* Big points clouds (1M+) are slow ([#1136](https://github.com/rerun-io/rerun/issues/1136))
* The data you want to visualize must fit in RAM.
  - See <https://github.com/rerun-io/rerun/issues/1138> for how to bound memory use
  - We plan on having a disk-based data store some time in the future
  - Additionally, Rerun is using more memory than it should at the moment ([#1242](https://github.com/rerun-io/rerun/pull/1242))


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
