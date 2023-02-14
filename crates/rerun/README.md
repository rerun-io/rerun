<h1 align="center">
  <a href="https://www.rerun.io/">
    <img alt="banner" src="https://user-images.githubusercontent.com/1148717/218142418-1d320929-6b7a-486e-8277-fbeef2432529.png">
  </a>
</h1>

[![Latest version](https://img.shields.io/crates/v/re_ws_comms.svg)](https://crates.io/crates/rerun)
[![Documentation](https://docs.rs/re_ws_comms/badge.svg)](https://docs.rs/rerun)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)
[![Discord](https://img.shields.io/discord/1062300748202921994?label=Rerun%20Discord)](https://discord.gg/Gcm8BbTaAj)

# Rerun Rust logging SDK
Rerun is an SDK for logging computer vision and robotics data paired with a visualizer for exploring that data over time. It lets you debug and understand the internal state and data of your systems with minimal code.

```shell
cargo add rerun
````

``` rust
rerun::MsgSender::new("points")
    .with_component(&points)?
    .with_component(&colors)?
    .send(&mut rerun::global_session())?;

rerun::MsgSender::new("image")
    .with_component(&[rerun::components::Tensor::from_image(image)?])?
    .send(&mut rerun::global_session())?;
```

<p align="center">
<img src="https://user-images.githubusercontent.com/1148717/218265704-1863c270-1422-48fe-9009-d67f8133c4cc.gif">
</p>

## Getting started
- [Examples](https://github.com/rerun-io/rerun/tree/latest/examples/rust)
- [High-level docs](http://rerun.io/docs)
- [Rust API docs](https://docs.rs/rerun/)

## Library
You can add the `rerun` crate to your project with `cargo add rerun`.

To get started, see [the examples](https://github.com/rerun-io/rerun/tree/latest/examples/rust).

## Binary
You can install the binary with `cargo install rerun`

This can act either as a server, a viewer, or both, depending on which options you use when you start it.

Running `rerun` with no arguments will start the viewer, waiting for an SDK to connect to it over TCP.

Run `rerun --help` for more.


### Running a web viewer
The web viewer is an experimental feature, but you can try it out with:

```sh
cargo install --features web rerun
rerun --web-viewer ../nyud.rrd
```
