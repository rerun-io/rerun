---
title: Installing the Rerun Viewer
order: -1
---

The [Rerun Viewer](../reference/viewer/overview.md) can be installed independent of the SDK language you're using.
Generally, you should make sure that your SDK version matches the version of the Viewer you're using to display any data you are logging.

There are many ways to install the viewer. Please pick whatever works best for your setup:

* `cargo binstall rerun-cli` - download binaries via [`cargo binstall`](https://github.com/cargo-bins/cargo-binstall)
* `cargo install rerun-cli` - build it from source (this requires Rust 1.74+)
* Download it from the [GitHub Release artifacts](https://github.com/rerun-io/rerun/releases/latest/)
* Together with the Rerun [Python SDK](python.md):
  * `pip3 install rerun-sdk` - download it via pip
  * `conda install -c conda-forge rerun-sdk` - download via Conda
  * `pixi global install rerun-sdk` - download it via [Pixi](https://prefix.dev/docs/pixi/overview)

In any case you should be able to run `rerun` afterwards to start the Viewer.
You'll be welcomed by an overview page that allows you to jump into some examples.
If you're facing any difficulties, don't hesitate to [open an issue](https://github.com/rerun-io/rerun/issues/new/choose) or [join the Discord server](https://discord.gg/PXtCgFBSmH).

The Rerun Viewer has built-in support for opening many kinds of files, and can be [extended to open any other file type](../howto/open-any-file.md) without needing to modify the Rerun codebase itself.

To start getting your own data logged & visualized in the viewer check one of the respective getting started guides:
* [Python](python.md)
* [C++](cpp.md)
* [Rust](rust.md)
