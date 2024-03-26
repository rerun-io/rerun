---
title: Installing Rerun
order: 2
---

## Installing the SDK

### Python

* `pip install rerun-sdk` via pip
* `conda install -c conda-forge rerun-sdk` via Conda

Either way this includes both the SDK & the viewer and you're ready to go!

### C++

If you're using CMake you can add the SDK to your project using `FetchContent`:

```cmake
include(FetchContent)
FetchContent_Declare(rerun_sdk URL
    https://github.com/rerun-io/rerun/releases/latest/download/rerun_cpp_sdk.zip)
FetchContent_MakeAvailable(rerun_sdk)
```
For more details see [Build & Distribution](https://ref.rerun.io/docs/cpp/stable/index.html#autotoc_md8) in the C++ reference documentation.
You'll additionally need to install the Viewer, see [below](#installing-the-viewer)

### Rust

Add the [Rerun crate](https://crates.io/crates/rerun) using `cargo add rerun`. You'll additionally need to install the Viewer, see [below](#installing-the-viewer).

## Installing the Viewer

The [Viewer](../reference/viewer/overview.md) can be installed independent of the library language you're using.
Make sure that your library version matches the version of the Viewer you're using.

There are many ways to install the viewer. Please pick whatever works best for your setup:

* Download it from the [GitHub Release artifacts](https://github.com/rerun-io/rerun/releases/latest/)
* Via Cargo
  * `cargo binstall rerun-cli` - download binaries via [`cargo binstall`](https://github.com/cargo-bins/cargo-binstall)
  * `cargo install rerun-cli` - build it from source (this requires Rust 1.74+)
* Together with the Rerun [Python SDK](./quick-start/python.md):
  * `pip3 install rerun-sdk` - download it via pip
  * `conda install -c conda-forge rerun-sdk` - download via Conda
  * `pixi global install rerun-sdk` - download it via [Pixi](https://prefix.dev/docs/pixi/overview)

In any case you should be able to run `rerun` afterwards to start the Viewer.
You'll be welcomed by an overview page that allows you to jump into some examples.
If you're facing any difficulties, don't hesitate to [open an issue](https://github.com/rerun-io/rerun/issues/new/choose) or [join the Discord server](https://discord.gg/PXtCgFBSmH).

The Rerun Viewer has built-in support for opening many kinds of files, and can be [extended to open any other file type](../howto/open-any-file.md) without needing to modify the Rerun codebase itself.

## Next Steps

To start getting your own data streamed to the viewer, check one of the respective getting started guides:
* [Python](./quick-start/python.md)
* [C++](./quick-start/cpp.md)
* [Rust](./quick-start/rust.md)
