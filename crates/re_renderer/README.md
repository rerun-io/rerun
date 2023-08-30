# The Rerun renderer

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_renderer.svg)](https://crates.io/crates/re_renderer)
[![Documentation](https://docs.rs/re_renderer/badge.svg)](https://docs.rs/re_renderer)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

A custom [wgpu](https://github.com/gfx-rs/wgpu/) based renderer tailored towards re_viewer's needs.
Nevertheless, it can be used standalone and comes with its own examples!

Some key features:
* Key primitives for visualization like lines and points are first class citizens
* Built with multiple independent views/cameras in mind
* WebGL compatible quality tier allows use in the browser without WebGPU support
* Hot shader reloading
* … and more to come!

Goals & philosophy:
* Handle fully dynamic data
  * assumes that most data may change every frame!
* Automatic resource re-use & caching
* Lazy loading whenever possible for best startup performance
* Run great both on the desktop and web
* No dependencies on `re_viewer` or rerun data store libraries
