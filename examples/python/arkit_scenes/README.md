<--[metadata]
title = "ARKit Scenes"
tags = ["2D", "3D", "depth", "mesh", "object-detection", "pinhole-camera"]
description = "Visualize the ARKitScenes dataset, which contains color+depth images, the reconstructed mesh and labeled bounding boxes."
thumbnail = "https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/480w.png"
thumbnail_dimensions = [480, 243]
channel = "main"
-->


<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/1200w.png">
  <img src="https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/full.png" alt="ARKit Scenes screenshot">
</picture>

This example visualizes the [ARKitScenes dataset](https://github.com/apple/ARKitScenes/) using Rerun. The dataset
contains color images, depth images, the reconstructed mesh, and labeled bounding boxes around furniture.

```bash
pip install -r examples/python/arkit_scenes/requirements.txt
python examples/python/arkit_scenes/main.py
```