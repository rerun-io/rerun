---
title: Viewer
order: 400
---

The [Viewer](../../reference/viewer/overview.md) can be installed independent of the library language you're using.
Make sure that your library version matches the version of the Viewer you're using, because [our data format is not yet stable across different versions](https://github.com/rerun-io/rerun/issues/6410).

There are many ways to install the viewer. Please pick whatever works best for your setup:

-   Download `rerun-cli` for your platform from the [GitHub Release artifacts](https://github.com/rerun-io/rerun/releases/latest/).
-   Via Cargo
    -   `cargo binstall rerun-cli` - download binaries via [`cargo binstall`](https://github.com/cargo-bins/cargo-binstall)
    -   `cargo install rerun-cli --locked` - build it from source (this requires Rust 1.92+)
-   Via Snap (_community maintained_)
    -   `snap install rerun` - download the viewer from the [Store](https://snapcraft.io/rerun).
-   Together with the Rerun [Python SDK](./python.md):
    -   `pip3 install rerun-sdk` - download it via pip
    -   `conda install -c conda-forge rerun-sdk` - download via Conda
    -   `pixi global install rerun-sdk` - download it via [Pixi](https://pixi.sh/latest/)

In any case you should be able to run `rerun` afterwards to start the Viewer.
You'll be welcomed by an overview page that allows you to jump into some examples.
If you're facing any difficulties, don't hesitate to [open an issue](https://github.com/rerun-io/rerun/issues/new/choose) or [join the Discord server](https://discord.gg/PXtCgFBSmH).

The Rerun Viewer has built-in support for opening many kinds of files, and can be [extended to open any other file type](../../getting-started/data-in/open-any-file.md) without needing to modify the Rerun codebase itself.
