# Rerun

[![Build Status](https://github.com/emilk/egui/workflows/CI/badge.svg)](https://github.com/emilk/egui/actions?workflow=CI)
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-MIT)
[![Apache](https://img.shields.io/badge/license-Apache-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-APACHE)
[![Discord](https://img.shields.io/discord/900275882684477440?label=Rerun%20Community%20Discord)](https://discord.gg/Gcm8BbTaAj)


Rerun is visualization infrastructure for computer vision.

You use one of our logging APIs (Python or Rust atm) to log rich data, such as images and point clouds, to the Rerun Viewer, where it is visualized live or after the fact.

```py
import rerun as rr

rr.init("my_app", spawn = True) # Spawn a Rerun Viewer and stream log events to it

rr.log_image("rgb_image", image)
rr.log_points("points", positions)
```

<!--- TODO(emilk): insert an image or gif here, preferably hosted elsewhere -->


## Documentation
- [Examples](examples)
- [Python API docs](https://rerun-io.github.io/rerun)
- [Rust getting-started guide](rerun_py/USAGE.md)
<!--- TODO(#1161): update doclinks
- [High-level documentation](http://rerun.io/docs)
- [Rust API docs](https://docs.rs/rerun/)
-->


## Installing the pre-release Python SDK
<!-- TODO(#1161): replace with `pip install rerun-sdk` -->
1. Download the correct `.whl` from [GitHub Releases](https://github.com/rerun-io/rerun/releases)
  (for Mac M1/M2, grab the "universal2" `.whl`)
2. Run `pip install rerun_sdk<...>.whl` (replace `<...>` with the actual filename)
3. Test it: `rerun --version`


## Installing the Rust SDK
Coming soon
<!-- TODO(#1161): `cargo add rerun` + `cargo install rerun` -->


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


## Business model
Rerun uses an open core model. Everything in this repository will stay open source and free (as in beer), forever. In the future, Rerun will offer a commercial product that builds on top of the core free project. 

The Rerun open source project targets the needs of individual developers. The commercial product targets the needs specific to teams that build and run computer vision and robotics products. 


# Development
* [`ARCHITECTURE.md`](ARCHITECTURE.md)
* [`BUILD.md`](BUILD.md)
* [`CODE_STYLE.md`](CODE_STYLE.md)
* [`CONTRIBUTING.md`](CONTRIBUTING.md)
* [`RELEASES.md`](RELEASES.md)
