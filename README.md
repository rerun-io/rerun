<h1 align="center">
  <a href="https://www.rerun.io/">
    <img alt="banner" src="https://user-images.githubusercontent.com/1148717/218142418-1d320929-6b7a-486e-8277-fbeef2432529.png">
  </a>
</h1>

<h1 align="center">
  <a href="https://pypi.org/project/rerun-sdk/">                          <img alt="PyPi"           src="https://img.shields.io/pypi/v/rerun-sdk.svg">                              </a>
  <a href="https://crates.io/crates/rerun">                               <img alt="crates.io"      src="https://img.shields.io/crates/v/rerun.svg">                                </a>
  <a href="https://github.com/rerun-io/rerun/blob/master/LICENSE-MIT">    <img alt="MIT"            src="https://img.shields.io/badge/license-MIT-blue.svg">                        </a>
  <a href="https://github.com/rerun-io/rerun/blob/master/LICENSE-APACHE"> <img alt="Apache"         src="https://img.shields.io/badge/license-Apache-blue.svg">                     </a>
  <a href="https://discord.gg/Gcm8BbTaAj">                                <img alt="Rerun Discord"  src="https://img.shields.io/discord/1062300748202921994?label=Rerun%20Discord"> </a>
</h1>

# Build time aware visualizations of multimodal data

Use the Rerun SDK (available in Python, Rust, and soon C++) to log data like images, tensors, point clouds, and text. Logs are streamed to the Rerun Viewer for live visualization or to file for later use.

```py
import rerun as rr  # pip install rerun-sdk

rr.init("rerun_example_app")

rr.connect()  # Connect to a remote viewer
# rr.spawn()  # Spawn a child process with a viewer and connect
# rr.save("recording.rrd")  # Stream all logs to disk

# Associate subsequent data with 42 on the “frame” timeline
rr.set_time_sequence("frame", 42))

# Log colored 3D points to the entity at `path/to/points`
rr.log("path/to/points", rr.Points3D(positions, colors=colors))
…
```

<p align="center">
  <img width="800" alt="Rerun Viewer" src="https://github.com/rerun-io/rerun/assets/2624717/c4900538-fc3a-43b8-841a-8d226e7b5a2e">
</p>

## Getting started
* **Python**: `pip install rerun-sdk` or on [`conda`](https://github.com/conda-forge/rerun-sdk-feedstock)
* **Rust**: `cargo add rerun`
* **C / C++**: [Coming soon](https://github.com/rerun-io/rerun/issues/2919)

### Rerun Viewer binary
Both the Python and Rust library can start the Rerun Viewer, but to stream log data over the network or load our `.rrd` data files you also need the `rerun` binary.

It can be installed with `pip install rerun-sdk` or with `cargo install rerun-cli`.

You should now be able to run `rerun --help` in any terminal.


### Documentation
- 📚 [High-level docs](http://rerun.io/docs)
- ⚙️ [Examples](examples)
- 🐍 [Python API docs](https://ref.rerun.io/docs/python)
- 🦀 [Rust API docs](https://docs.rs/rerun/)
- ⁉️ [Troubleshooting](https://www.rerun.io/docs/getting-started/troubleshooting)


## Status
We are in active development.
There are many features we want to add, and the API is still evolving.
_Expect breaking changes!_

Some shortcomings:
* Big points clouds (1M+) are slow ([#1136](https://github.com/rerun-io/rerun/issues/1136))
* The data you want to visualize must fit in RAM.
  - See <https://www.rerun.io/docs/howto/limit-ram> for how to bound memory use
  - We plan on having a disk-based data store some time in the future
* The Rust library takes a long time to compile
  - We have way too many big dependencies, and we are planning on improving the situation ([#1316](https://github.com/rerun-io/rerun/pull/1316))


## Business model
Rerun uses an open-core model. Everything in this repository will stay open source and free (both as in beer and as in freedom).
In the future, Rerun will offer a commercial product that builds on top of the core free project.

The Rerun open source project targets the needs of individual developers.
The commercial product targets the needs specific to teams that build and run computer vision and robotics products.


# Development
* [`ARCHITECTURE.md`](ARCHITECTURE.md)
* [`BUILD.md`](BUILD.md)
* [`rerun_py/README.md`](rerun_py/README.md) - build instructions for Python SDK
* [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)
* [`CODE_STYLE.md`](CODE_STYLE.md)
* [`CONTRIBUTING.md`](CONTRIBUTING.md)
* [`RELEASES.md`](RELEASES.md)


## Installing a pre-release Python SDK

1. Download the correct `.whl` from [GitHub Releases](https://github.com/rerun-io/rerun/releases)
2. Run `pip install rerun_sdk<…>.whl` (replace `<…>` with the actual filename)
3. Test it: `rerun --version`
