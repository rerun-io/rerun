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
* No dependencies on `re_viewer` or Rerun chunk store libraries


## Debugging

### Shader

#### Iterating

In debug mode shaders are live-reloaded, if built from the Rerun workspace.
If a failure occurs during live-reload, an error is logged and the previous shader is kept.

#### Inspecting final source

If `RERUN_WGSL_SHADER_DUMP_PATH` is set, all readily stitched (import resolve) and patched
wgsl shaders will be written to the specified directory.

Often you're also interested in the Naga translated shader. This can be done easily from command line using
```sh
cargo install naga-cli --all-features
```

Example for translating a wgsl fragment shader to GL as used on WebGL:
```sh
naga ./wgsl_dump/rectangle_fs.wgsl ./wgsl_dump/rectangle_fs.frag --entry-point fs_main --profile es300
```
Example for translating a wgsl vertex shader to GL as used on WebGL:
```sh
naga ./wgsl_dump/rectangle_vs.wgsl ./wgsl_dump/rectangle_vs.vert --entry-point vs_main --profile es300
```
Note that a single shader entry point from wgsl maps to a single frag/vert file!

Example for translating a wgsl to MSL as used on MacOS.
Note that a single metal file maps to a single wgsl file.
```sh
naga ./wgsl_dump/rectangle_fs.wgsl ./wgsl_dump/rectangle_fs.metal
```

