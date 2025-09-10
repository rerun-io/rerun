---
title: Opening files
order: 4
---

The Rerun Viewer and SDK have built-in support for opening many kinds of files, and can be extended to support any other file type without needing to modify the Rerun codebase itself.

The Viewer can load files in 3 different ways:

-   via CLI arguments (e.g. `rerun myfile.jpeg`),
-   using drag-and-drop,
-   using the open dialog in the Rerun Viewer.

All these file loading methods support loading a single file, many files at once (e.g. `rerun myfiles/*`), or even folders.

⚠ Drag-and-drop of folders does [not yet work](https://github.com/rerun-io/rerun/issues/4528) on the web version of the Rerun Viewer ⚠

The following data types have built-in support in the Rerun Viewer and SDK:

-   Native Rerun files: `rrd`
-   3D models: `gltf`, `glb`, `obj`, `stl`
-   Images: `avif`, `bmp`, `dds`, `exr`, `farbfeld`, `ff`, `gif`, `hdr`, `ico`, `jpeg`, `jpg`, `pam`, `pbm`, `pgm`, `png`, `ppm`, `tga`, `tif`, `tiff`, `webp`
-   Point clouds: `ply`
-   Text files: `md`, `txt`
-   [LeRobot](https://huggingface.co/docs/lerobot/index) datasets: `directory`

With the exception of `rrd` files that can be streamed from an HTTP URL (e.g. `rerun https://demo.rerun.io/version/latest/examples/dna/data.rrd`), we only support loading files from the local filesystem for now, with [plans to make this generic over any URI and protocol in the future](https://github.com/rerun-io/rerun/issues/4525).

## Logging file contents from the SDK

To log the contents of a file from the SDK you can use the `log_file_from_path` and `log_file_from_contents` methods ([C++](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#a8f253422a7adc2a19b89d1538c05bcac), [Python](https://ref.rerun.io/docs/python/stable/common/other_classes_and_functions/#rerun.log_file_from_path), [Rust](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.log_file_from_path)) and the associated examples ([C++](https://github.com/rerun-io/rerun/blob/main/examples/cpp/log_file/main.cpp), [Python](https://github.com/rerun-io/rerun/blob/main/examples/python/log_file/log_file.py), [Rust](https://github.com/rerun-io/rerun/blob/main/examples/rust/log_file/src/main.rs)).

Note: when calling these APIs from the SDK, the data will be loaded by the process running the SDK, not the Viewer!

snippet: tutorials/log-file
