---
title: Opening any file
order: 0
---

Rerun can open many kinds of files by default and can be extended to support arbitrarily many more.

Files can be loaded in 3 different ways:
- via the Rerun CLI (e.g. `rerun myfile.jpeg`),
- using drag-and-drop,
- using the open dialog in the Rerun Viewer.

All these file loading methods support loading a single file, many files at once (e.g. `rerun myfiles/*`), or even folders.

⚠ Drag-and-drop of folders does not yet work on the web version of the Rerun Viewer ⚠

The [`DataLoader`](https://github.com/rerun-io/rerun/blob/main/crates/re_data_source/src/data_loader/mod.rs) trait takes care of loading files.
Rerun ships with a bunch of builtin implementations for that trait, which handle the following filetypes:
- Native Rerun files: `rrd`
- 3D models: `gltf`, `glb`, `obj`
- Images: `avif`, `bmp`, `dds`, `exr`, `farbfeld`, `ff`, `gif`, `hdr`, `ico`, `jpeg`, `jpg`, `pam`, `pbm`, `pgm`, `png`, `ppm`, `tga`, `tif`, `tiff`, `webp`.
- Point clouds: `ply`.
- Text files: `md`, `txt`.

With the exception of `rrd` files that can be streamed from an HTTP URL (e.g. `rerun https://demo.rerun.io/version/latest/examples/dna/data.rrd`), we only support loading files from the local filesystem for now, with [plans to make this generic over any URI and protocol in the future](https://github.com/rerun-io/rerun/issues/4525).

## Adding support for arbitrary filetypes

If the builtin `DataLoader`s don't cover your needs, we expose two ways of implementing and registering your own loaders.

### External data-loaders

The easiest way to do so is by implementing what we call an "external loader": an executable -- written in any language that the Rerun SDK ships for -- that is available on your `$PATH`.

This executable is just a vanilla Rerun logger.
It will be called by the Rerun Viewer when the user opens a file, and be passed the path to that file.
From there, it can log data as usual, using the [`stdout` logging sink](https://www.rerun.io/docs/reference/sdk-operating-modes).

The Rerun Viewer will then automatically load the data streamed to the external loader's standard output.

Check out our examples for [C++](https://github.com/rerun-io/rerun/tree/main/examples/cpp/external_data_loader), [Python](https://github.com/rerun-io/rerun/tree/main/examples/python/external_data_loader) and [Rust](https://github.com/rerun-io/rerun/tree/main/examples/rust/external_data_loader) that cover every steps in details.

### Custom data-loaders

Another Rust-specific approach is to implement the `DataLoader` trait yourself and register it in the Rerun Viewer.

To do so, you'll need to import `rerun` as a library, register your `DataLoader` and then starts the viewer from code.

Check out our [example](https://github.com/rerun-io/rerun/tree/main/examples/rust/custom_data_loader) that cover all these steps in details.
