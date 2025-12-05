---
title: Loading URDF models
order: 600
---

Rerun features a built-in [data-loader](https://rerun.io/docs/reference/data-loaders/overview) for [URDF](https://en.wikipedia.org/wiki/URDF) files.

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/urdf-viewer/ebdefa158ab6f26f9dc1cb1924fce4b846fe8db2/full.png" alt="A robot model loaded from an URDF file visualized in Rerun.">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/urdf-viewer/ebdefa158ab6f26f9dc1cb1924fce4b846fe8db2/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/urdf-viewer/ebdefa158ab6f26f9dc1cb1924fce4b846fe8db2/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/urdf-viewer/ebdefa158ab6f26f9dc1cb1924fce4b846fe8db2/1024w.png">
</picture>

## Overview

Using a `URDF` in Rerun only requires you to load the file with the logging API.
This will automatically invoke the data-loader, which will take care of:
* resolving paths to meshes
* loading meshes and shapes as Rerun entities
* loading the joint transforms and associated frame IDs of links

Once that is done, the joints can be updated by sending `Transform3D`s, where you have to set the `parent_frame` and `child_frame` fields explicitly to each joint's specific frame IDs.

> ⚠️ Note: previous versions required you to send transforms with _implicit_ frame IDs, i.e. having to send each joint transform on a specific entity path.
> This was dropped in favor of _named_ frame IDs, which is more in line with ROS and allows you to send all transform updates on one entity (e.g. a `transforms` entity like in the example below).

## Example

Here is an example that demonstrates how to load and update a `URDF` with the Python SDK:

snippet: howto/load_urdf

For similar code in Rust, we have a full example [here](github.com/rerun-io/rerun/tree/main/examples/rust/animated_urdf).

## References

* [🐍 Python `log_file_from_path`](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.log_file_from_path)
* [🦀 Rust `log_file_from_path`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.log_file_from_path)
* [🌊 C++ `log_file_from_path`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#a20798d7ea74cce5c8174e5cacd0a2c47)
