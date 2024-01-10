---
title: Open any file
order: -10
---

The Rerun Viewer has built-in support for opening many kinds of files, and can be extended to open any other file type without needing to modify the Rerun codebase itself.

The viewer can load files in 3 different ways:
- via CLI arguments (e.g. `rerun myfile.jpeg`),
- using drag-and-drop,
- using the open dialog in the Rerun Viewer.

All these file loading methods support loading a single file, many files at once (e.g. `rerun myfiles/*`), or even folders.

⚠ Drag-and-drop of folders does [not yet work](https://github.com/rerun-io/rerun/issues/4528) on the web version of the Rerun Viewer ⚠

The following file types have built-in support in the Rerun Viewer:
- Native Rerun files: `rrd`
- 3D models: `gltf`, `glb`, `obj`
- Images: `avif`, `bmp`, `dds`, `exr`, `farbfeld`, `ff`, `gif`, `hdr`, `ico`, `jpeg`, `jpg`, `pam`, `pbm`, `pgm`, `png`, `ppm`, `tga`, `tif`, `tiff`, `webp`.
- Point clouds: `ply`.
- Text files: `md`, `txt`.

With the exception of `rrd` files that can be streamed from an HTTP URL (e.g. `rerun https://demo.rerun.io/version/latest/examples/dna/data.rrd`), we only support loading files from the local filesystem for now, with [plans to make this generic over any URI and protocol in the future](https://github.com/rerun-io/rerun/issues/4525).

## Adding support for arbitrary filetypes

Internally, the [`DataLoader`](https://docs.rs/re_data_source/latest/re_data_source/trait.DataLoader.html) trait takes care of loading files into the Viewer.

There are 3 broad kinds of `DataLoader`s: _builtin_, _external_ and _custom_.
_External_ and _custom_ are the two ways of extending the file loading system that we'll describe below.

When a user attempts to open a file in the Viewer, **all** known `DataLoader`s are notified of the path to be opened, unconditionally.
This gives `DataLoader`s maximum flexibility to decide what files they are interested in, as opposed to e.g. only being able to look at a file's extension.

Once notified, a `DataLoader` can return a [`DataLoaderError::Incompatible`](https://docs.rs/re_data_source/latest/re_data_source/enum.DataLoaderError.html#variant.Incompatible) error to indicate that it doesn't support a given file type.
If, and only if, all loaders known to the Viewer return an `Incompatible` error code, then an error message is shown to the user indicating that this file type is not (_yet_) supported.

In these instances of unsupported files, we expose two ways of implementing and registering your `DataLoader`s, explained below.

### External data-loaders

The easiest way to create your own `DataLoader` is by implementing what we call an "external loader": a stand alone executable written in any language that the Rerun SDK ships for. Any executable on your `$PATH` with a name that starts with `rerun-loader-` will be treated as a `DataLoader`.

This executable takes a file path as a command line argument and outputs Rerun logs on `stdout`.
It will be called by the Rerun Viewer when the user opens a file, and be passed the path to that file.
From there, it can log data as usual, using the [`stdout` logging sink](../reference/sdk-operating-modes.md#standard-inputoutput).

The Rerun Viewer will then automatically load the data streamed to the external loader's standard output.

<picture>
  <img src="https://static.rerun.io/data-loader-external-overview/97e978000c709b78290f50d52c229a91f7543648/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/data-loader-external-overview/97e978000c709b78290f50d52c229a91f7543648/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/data-loader-external-overview/97e978000c709b78290f50d52c229a91f7543648/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/data-loader-external-overview/97e978000c709b78290f50d52c229a91f7543648/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/data-loader-external-overview/97e978000c709b78290f50d52c229a91f7543648/1200w.png">
</picture>

Like any other `DataLoader`, an external loader will be notified of all file openings, unconditionally.
To indicate that it does not support a given file, the loader has to exit with a [dedicated status code](https://docs.rs/rerun/latest/rerun/constant.EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE.html).

Check out our examples for [C++](https://github.com/rerun-io/rerun/tree/main/examples/cpp/external_data_loader), [Python](https://github.com/rerun-io/rerun/tree/main/examples/python/external_data_loader) and [Rust](https://github.com/rerun-io/rerun/tree/main/examples/rust/external_data_loader) that cover every steps in details.

### Custom data-loaders

Another Rust-specific approach is to implement the `DataLoader` trait yourself and register it in the Rerun Viewer.

To do so, you'll need to import `rerun` as a library, register your `DataLoader` and then start the viewer from code.

Check out our [example](https://github.com/rerun-io/rerun/tree/main/examples/rust/custom_data_loader) that cover all these steps in details.
