<h1 align="center">
  <a href="https://www.rerun.io/">
    <img alt="banner" src="https://user-images.githubusercontent.com/1148717/218142418-1d320929-6b7a-486e-8277-fbeef2432529.png">
  </a>
</h1>

<h1 align="center">
  <a href="https://crates.io/crates/rerun">                               <img alt="Latest version" src="https://img.shields.io/crates/v/rerun.svg">                               </a>
  <a href="https://docs.rs/rerun">                                        <img alt="Documentation"  src="https://docs.rs/rerun/badge.svg">                                         </a>
  <a href="https://github.com/rerun-io/rerun/blob/master/LICENSE-MIT">    <img alt="MIT"            src="https://img.shields.io/badge/license-MIT-blue.svg">                        </a>
  <a href="https://github.com/rerun-io/rerun/blob/master/LICENSE-APACHE"> <img alt="Apache"         src="https://img.shields.io/badge/license-Apache-blue.svg">                     </a>
  <a href="https://discord.gg/Gcm8BbTaAj">                                <img alt="Rerun Discord"  src="https://img.shields.io/discord/1062300748202921994?label=Rerun%20Discord"> </a>
</h1>

# Rerun Rust logging SDK
Rerun is an SDK for logging computer vision and robotics data paired with a visualizer for exploring that data over time. It lets you debug and understand the internal state and data of your systems with minimal code.

```shell
cargo add rerun
````

```rust
let rec = rerun::RecordingStream::global(rerun::StoreKind::Recording)?;
rec.log("points", &rerun::archetypes::Points3D::new(points).with_colors(colors))?;
rec.log("image", &rerun::archetypes::Image::new(image))?;
```

<p align="center">
  <img width="800" alt="Rerun Viewer" src="https://user-images.githubusercontent.com/1148717/218763490-f6261ecd-e19e-4520-9b25-446ce1ee6328.png">
</p>

## Getting started
- [Examples](https://github.com/rerun-io/rerun/tree/latest/examples/rust)
- [High-level docs](http://rerun.io/docs)
- [Rust API docs](https://docs.rs/rerun/)
- [Troubleshooting](https://www.rerun.io/docs/getting-started/troubleshooting)

## Library
You can add the `rerun` crate to your project with `cargo add rerun`.

To get started, see [the examples](https://github.com/rerun-io/rerun/tree/latest/examples/rust).

## Binary
You can install the binary with `cargo install rerun-cli`

This can act either as a server, a viewer, or both, depending on which options you use when you start it.

Running `rerun` with no arguments will start the viewer, waiting for an SDK to connect to it over TCP.

Run `rerun --help` for more.


### Running a web viewer
The web viewer is an experimental feature, but you can try it out with:

```sh
rerun --web-viewer path/to/file.rrd
```
