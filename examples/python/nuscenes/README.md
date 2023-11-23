---
title: nuScenes
python: https://github.com/rerun-io/rerun/blob/latest/examples/python/nuscenes/main.py
tags: [lidar, 3D, 2D, object-detection, pinhole-camera]
description: "Visualize the nuScenes dataset, which contains lidar data and color images, and labeled bounding boxes."
thumbnail: https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/480w.png
thumbnail_dimensions: [480, 243]
demo: true
---

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/1200w.png">
  <img src="https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/full.png" alt="ARKit Scenes screenshot">
</picture>

This example visualizes the [nuScenes dataset](https://www.nuscenes.org/) using Rerun. The dataset
contains lidar data, color images, and labeled bounding boxes.

```bash
pip install -r examples/python/nuscenes/requirements.txt
python examples/python/nuscenes/main.py
```
