---
title: ARKit Scenes
python: https://github.com/rerun-io/rerun/blob/latest/examples/python/arkit_scenes/main.py
tags: [2D, 3D, depth, mesh, object-detection, pinhole-camera]
description: "Visualize the ARKitScenes dataset, which contains color+depth images, the reconstructed mesh and labeled bounding boxes."
thumbnail: https://static.rerun.io/8b90a80c72b27fad289806b7e5dff0c9ac97e87c_arkit_scenes_480w.png
build_args: []
---

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/8b90a80c72b27fad289806b7e5dff0c9ac97e87c_arkit_scenes_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/4096dbc9d30f098b4b01acd064927d2374ee48f5_arkit_scenes_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/2e4b62a595cf409d8bcbe6ded0d4bee3d7c54d16_arkit_scenes_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/9f565fa5205585da989636781fa9acf864a38f51_arkit_scenes_1200w.png">
  <img src="https://static.rerun.io/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc_arkit_scenes_full.png" alt="ARKit Scenes screenshot">
</picture>


Visualizes the [ARKitScenes dataset](https://github.com/apple/ARKitScenes/) using the Rerun SDK.
The dataset contains color+depth images, the reconstructed mesh and labeled bounding boxes around furniture.

```bash
pip install -r examples/python/arkit_scenes/requirements.txt
python examples/python/arkit_scenes/main.py
```
