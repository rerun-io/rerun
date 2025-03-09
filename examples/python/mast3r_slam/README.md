<!--[metadata]
title = "Mast3r SLAM - Real-Time Dense SLAM with 3D Reconstruction Priors"
tags = ["2D", "3D", "Pinhole camera", "Time series", "SLAM"]
source = "https://github.com/rerun-io/mast3r-slam"
thumbnail = "https://static.rerun.io/thumbnail/3659cc28fb5ab6173f930e26dd8158f1638eb284/480w.png"
thumbnail_dimensions = [480, 301]
-->

https://vimeo.com/1064055355?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=2802:1790

## Background

Mast3r-slam is a realtime monocular slam system that is based on Mast3r, a two view 3D reconstruction and matching prior. Equipped with this strong prior, the system is robust on in-the-wild video sequences despite making no assumption on a fixed or parametric camera model beyond a unique camera centre. It introduces efficient methods for pointmap matching, camera tracking and local fusion, graph construction and loop closure, and second-order global optimisation. With known calibration, a simple modification to the system achieves state-of-the-art performance across various benchmarks.

## Run the code

This is an external example. Check the [repository](https://github.com/rerun-io/mast3r-slam) for more information on how to run the code.

TLDR: make sure you have the [Pixi package manager](https://pixi.sh/latest/#installation) installed and run
```
git clone https://github.com/rerun-io/mast3r-slam.git
cd mast3r-slam
pixi run example-base
```
