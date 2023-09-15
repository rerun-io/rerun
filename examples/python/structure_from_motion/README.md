---
title: Structure from Motion
python: https://github.com/rerun-io/rerun/tree/latest/examples/python/structure_from_motion/main.py
tags: [2D, 3D, colmap, pinhole-camera, time-series]
description: "Visualize a sparse reconstruction by COLMAP, a general-purpose Structure-from-Motion and Multi-View Stereo pipeline."
thumbnail: https://static.rerun.io/033edff752f86bcdc9a81f7877e0b4411ff4e6c5_structure_from_motion_480w.png
thumbnail_dimensions: [480, 275]
build_args: ["--dataset=colmap_fiat", "--resize=800x600"]
---

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/033edff752f86bcdc9a81f7877e0b4411ff4e6c5_structure_from_motion_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/29f207025a6c5a63e487f95fc6098a4f1f8d9ca3_structure_from_motion_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/6b7914b63f909f2ac5b23530a7d7363178b331cb_structure_from_motion_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/7047a851275c94c2a7e018bd7230dac96c0cea09_structure_from_motion_1200w.png">
  <img src="https://static.rerun.io/b17f8824291fa1102a4dc2184d13c91f92d2279c_structure_from_motion_full.png" alt="Structure From Motion example screenshot">
</picture>

An example using Rerun to log and visualize the output of COLMAP's sparse reconstruction.

[COLMAP](https://colmap.github.io/index.html) is a general-purpose Structure-from-Motion (SfM) and Multi-View Stereo (MVS) pipeline with a graphical and command-line interface.

In this example a short video clip has been processed offline by the COLMAP pipeline, and we use Rerun to visualize the individual camera frames, estimated camera poses, and resulting point clouds over time.


```bash
pip install -r examples/python/structure_from_motion/requirements.txt
python examples/python/structure_from_motion/main.py
```
