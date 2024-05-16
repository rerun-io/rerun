<!--[metadata]
title = "URDF loader"
source = "https://github.com/rerun-io/rerun-loader-python-example-urdf"
tags = ["3D", "Mesh", "Loader"]
thumbnail = "https://static.rerun.io/urdf_loader/9c04fbb376cd4f7498628a98593035c6da0f17fb/480w.png"
thumbnail_dimensions = [480, 480]
-->

<picture>
  <img src="https://static.rerun.io/urdf_loader/fe6730519ceb0f73040fce8aa7cc89e773bafe5c/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/urdf_loader/fe6730519ceb0f73040fce8aa7cc89e773bafe5c/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/urdf_loader/fe6730519ceb0f73040fce8aa7cc89e773bafe5c/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/urdf_loader/fe6730519ceb0f73040fce8aa7cc89e773bafe5c/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/urdf_loader/fe6730519ceb0f73040fce8aa7cc89e773bafe5c/1200w.png">
</picture>


## Overview

This is an example data-loader plugin that lets you view [URDF](https://wiki.ros.org/urdf) files. It uses the [external data loader mechanism](https://www.rerun.io/docs/reference/data-loaders/overview#external-dataloaders) to add this capability to the Rerun Viewer without modifying the Viewer itself.

This example is written in Python, and uses [urdf_parser_py](https://github.com/ros/urdf_parser_py/tree/ros2) to read the files. ROS package-relative paths support both ROS 1 and ROS 2-based resolving.

## Installing the plug-in

The [repository](https://github.com/rerun-io/rerun-loader-python-example-urdf) has detailed installation instruction. In a nutshell, the easiest is to use `pipx`:

```
pipx install git+https://github.com/rerun-io/rerun-loader-python-example-urdf.git
pipx ensurepath
```


## Try it out

To try the plug-in, first download the provided example URDF:

```bash
curl -OL https://github.com/rerun-io/rerun-loader-python-example-urdf/raw/main/example.urdf
```

Then you can open the Viewer and open the file using drag-and-drop or the open dialog, or you can open it directly from the terminal:

```bash
rerun example.urdf
```
