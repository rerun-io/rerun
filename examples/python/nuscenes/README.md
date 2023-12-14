---
title: nuScenes
python: https://github.com/rerun-io/rerun/blob/latest/examples/python/nuscenes/main.py?speculative-link
tags: [lidar, 3D, 2D, object-detection, pinhole-camera]
description: "Visualize the nuScenes dataset including lidar, radar, images, and bounding boxes."
thumbnail: https://static.rerun.io/nuscenes/64a50a9d67cbb69ae872551989ee807b195f6b5d/480w.png
thumbnail_dimensions: [480, 282]
channel: nightly
---

<picture>
  <img src="https://static.rerun.io/nuscenes/64a50a9d67cbb69ae872551989ee807b195f6b5d/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/nuscenes/64a50a9d67cbb69ae872551989ee807b195f6b5d/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/nuscenes/64a50a9d67cbb69ae872551989ee807b195f6b5d/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/nuscenes/64a50a9d67cbb69ae872551989ee807b195f6b5d/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/nuscenes/64a50a9d67cbb69ae872551989ee807b195f6b5d/1200w.png">
</picture>

This example visualizes the [nuScenes dataset](https://www.nuscenes.org/) using Rerun. The dataset
contains lidar data, radar data, color images, and labeled bounding boxes.

```bash
pip install -r examples/python/nuscenes/requirements.txt
python examples/python/nuscenes/main.py
```
