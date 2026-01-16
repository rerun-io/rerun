<h1 align="center">
  <a href="https://www.rerun.io/">
    <img alt="banner" src="https://user-images.githubusercontent.com/1148717/218142418-1d320929-6b7a-486e-8277-fbeef2432529.png">
  </a>
</h1>

<h1 align="center">
  <a href="https://pypi.org/project/rerun-sdk/">                        <img alt="PyPi"           src="https://img.shields.io/pypi/v/rerun-sdk.svg">                              </a>
  <a href="https://crates.io/crates/rerun">                             <img alt="crates.io"      src="https://img.shields.io/crates/v/rerun.svg">                                </a>
  <a href="https://github.com/rerun-io/rerun/blob/main/LICENSE-MIT">    <img alt="MIT"            src="https://img.shields.io/badge/license-MIT-blue.svg">                        </a>
  <a href="https://github.com/rerun-io/rerun/blob/main/LICENSE-APACHE"> <img alt="Apache"         src="https://img.shields.io/badge/license-Apache-blue.svg">                     </a>
  <a href="https://discord.gg/Gcm8BbTaAj">                              <img alt="Rerun Discord"  src="https://img.shields.io/discord/1062300748202921994?label=Rerun%20Discord"> </a>
</h1>

# Time-aware multimodal data stack and visualizations
Rerun is building the multimodal data stack to model, ingest, store, query and view robotics-style data.
It's used in areas like robotics, spatial and embodied AI, generative media, industrial processing, simulation, security, and health.

Rerun is easy to use!
Use the Rerun SDK (available for C++, Python and Rust) to log data like images, tensors, point clouds, and text.
Logs are streamed to the Rerun Viewer for live visualization or to file for later use.
You can also query the logged data through [our dataframe API](https://rerun.io/docs/howto/dataframe-api).

[Get started](#getting-started) in minutes – no account needed.

* [Run the Rerun Viewer in your browser](https://www.rerun.io/viewer)
* [Read about what Rerun is and who it is for](https://www.rerun.io/docs/getting-started/what-is-rerun)

### A short taste
```py
import rerun as rr  # pip install rerun-sdk

rr.init("rerun_example_app")

rr.spawn()  # Spawn a child process with a viewer and connect
# rr.save("recording.rrd")  # Stream all logs to disk
# rr.connect_grpc()  # Connect to a remote viewer

# Associate subsequent data with 42 on the “frame” timeline
rr.set_time("frame", sequence=42)

# Log colored 3D points to the entity at `path/to/points`
rr.log("path/to/points", rr.Points3D(positions, colors=colors))
…
```

<p align="center">
  <picture>
    <img src="https://static.rerun.io/opf_screenshot/bee51040cba93c0bae62ef6c57fa703704012a41/full.png" alt="">
    <source media="(max-width: 480px)" srcset="https://static.rerun.io/opf_screenshot/bee51040cba93c0bae62ef6c57fa703704012a41/480w.png">
    <source media="(max-width: 768px)" srcset="https://static.rerun.io/opf_screenshot/bee51040cba93c0bae62ef6c57fa703704012a41/768w.png">
    <source media="(max-width: 1024px)" srcset="https://static.rerun.io/opf_screenshot/bee51040cba93c0bae62ef6c57fa703704012a41/1024w.png">
    <source media="(max-width: 1200px)" srcset="https://static.rerun.io/opf_screenshot/bee51040cba93c0bae62ef6c57fa703704012a41/1200w.png">
  </picture>
</p>

## Getting started
* [**C++**](https://www.rerun.io/docs/getting-started/data-in/cpp)
* [**Python**](https://www.rerun.io/docs/getting-started/data-in/python): `pip install rerun-sdk` or on [`conda`](https://github.com/conda-forge/rerun-sdk-feedstock)
* [**Rust**](https://www.rerun.io/docs/getting-started/data-in/rust): `cargo add rerun`

### Installing the Rerun Viewer binary
To stream log data over the network or load our `.rrd` data files you also need the `rerun` binary.
It can be installed with `pip install rerun-sdk` or with `cargo install rerun-cli --locked --features nasm` (see note below).
Note that only the Python SDK comes bundled with the Viewer whereas C++ & Rust always rely on a separate install.

**Note**: the `nasm` Cargo feature requires the [`nasm`](https://github.com/netwide-assembler/nasm) CLI to be installed and available in your path.
Alternatively, you may skip enabling this feature, but this may result in inferior video decoding performance.

You should now be able to run `rerun --help` in any terminal.


### Documentation
- 📚 [High-level docs](http://rerun.io/docs)
- ⏃ [Loggable Types](https://www.rerun.io/docs/reference/types)
- ⚙️ [Examples](http://rerun.io/examples)
- 📖 [Code snippets](./docs/snippets/INDEX.md)
- 🌊 [C++ API docs](https://ref.rerun.io/docs/cpp)
- 🐍 [Python API docs](https://ref.rerun.io/docs/python)
- 🦀 [Rust API docs](https://docs.rs/rerun/)
- ⁉️ [Troubleshooting](https://www.rerun.io/docs/getting-started/troubleshooting)


## Status
We are in active development.
There are many features we want to add, and the API is still evolving.
_Expect breaking changes!_

Some shortcomings:
* [The viewer slows down when there are too many entities](https://github.com/rerun-io/rerun/issues/7115)
* [We don't support transparency yet](https://github.com/rerun-io/rerun/issues/1611)
* The data you want to visualize must fit in RAM
  - See <https://www.rerun.io/docs/howto/limit-ram> for how to bound memory use.
  - We plan on having a disk-based data store some time in the future.
* [Multi-million point clouds can be slow](https://github.com/rerun-io/rerun/issues/1136)


## What is Rerun for?

Rerun is built to help you understand and improve complex processes that include rich multimodal data, like 2D, 3D, text, time series, tensors, etc.
It is used in many industries, including robotics, simulation, computer vision,
or anything that involves a lot of sensors or other signals that evolve over time.

### Example use case
Say you're building a vacuum cleaning robot and it keeps running into walls. Why is it doing that? You need some tool to debug it, but a normal debugger isn't gonna be helpful. Similarly, just logging text won't be very helpful either. The robot may log "Going through doorway" but that won't explain why it thinks the wall is a door.

What you need is a visual and temporal debugger, that can log all the different representations of the world the robots holds in its little head, such as:

* RGB camera feed
* depth images
* lidar scan
* segmentation image (how the robot interprets what it sees)
* its 3D map of the apartment
* all the objects the robot has detected (or thinks it has detected), as 3D shapes in the 3D map
* its confidence in its prediction
* etc

You also want to see how all these streams of data evolve over time so you can go back and pinpoint exactly what went wrong, when and why.

Maybe it turns out that a glare from the sun hit one of the sensors in the wrong way, confusing the segmentation network leading to bad object detection. Or maybe it was a bug in the lidar scanning code. Or maybe the robot thought it was somewhere else in the apartment, because its odometry was broken. Or it could be one of a thousand other things. Rerun will help you find out!

But seeing the world from the point of the view of the robot is not just for debugging - it will also give you ideas on how to improve the algorithms, new test cases to set up, or datasets to collect. It will also let you explain the brains of the robot to your colleagues, boss, and customers. And so on. Seeing is believing, and an image is worth a thousand words, and multimodal temporal logging is worth a thousand images :)

While seeing and understanding your data is core to making progress in robotics, there is one more thing:
You can also use the data you collected for visualization to create new datasets for training and evaluating the models and algorithms that run on your robot.
Rerun provides query APIs to make it easy to extract clean datasets from your recording for exactly that purpose.

Of course, Rerun is useful for much more than just robots. Any time you have any form of sensors, or 2D or 3D state evolving over time, Rerun is a great tool.


## Business model
Rerun uses an open-core model. Everything in this repository will stay open source and free (both as in beer and as in freedom).

We are also building a commercial data platform.
Right now that is only available for a few select design partners.
[Click here if you're interested](https://rerun.io/pricing).

The Rerun open source project targets the needs of individual developers.
The commercial product targets the needs specific to teams that build and run computer vision and robotics products.

## How to cite Rerun

When using Rerun in your research, please cite it to acknowledge its contribution to your work. This can be done by
including a reference to Rerun in the software or methods section of your paper.

Suggested citation format:

```bibtex
@software{RerunSDK,
  title = {Rerun: A Visualization SDK for Multimodal Data},
  author = {{Rerun Development Team}},
  url = {https://www.rerun.io},
  version = {insert version number},
  date = {insert date of usage},
  year = {2024},
  publisher = {{Rerun Technologies AB}},
  address = {Online},
  note = {Available from https://www.rerun.io/ and https://github.com/rerun-io/rerun}
}
```

Please replace "insert version number" with the version of Rerun you used and "insert date of usage" with the date(s)
you used the tool in your research.
This citation format helps ensure that Rerun's development team receives appropriate credit for their work and
facilitates the tool's discovery by other researchers.

# Development
* [`ARCHITECTURE.md`](ARCHITECTURE.md)
* [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)
* [`CODE_STYLE.md`](CODE_STYLE.md)
* [`CONTRIBUTING.md`](CONTRIBUTING.md)
* [`BUILD.md`](BUILD.md)
* [`rerun_py/README.md`](rerun_py/README.md) - instructions for Python SDK
* [`rerun_cpp/README.md`](rerun_cpp/README.md) - instructions for C++ SDK


## Installing a pre-release Python SDK

1. Download the correct `.whl` from [GitHub Releases](https://github.com/rerun-io/rerun/releases)
2. Run `pip install rerun_sdk<…>.whl` (replace `<…>` with the actual filename)
3. Test it: `rerun --version`
