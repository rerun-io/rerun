<!--[metadata]
title = "RGBD"
tags = ["2D", "3D", "depth", "nyud", "pinhole-camera"]
description = "Visualizes an example recording from the NYUD dataset with RGB and Depth channels."
thumbnail = "https://static.rerun.io/rgbd/2fde3a620adc8bd9a5680260f0792d16ac5498bd/480w.png"
thumbnail_dimensions = [480, 480]
channel = "release"
build_args = ["--frames=300"]
-->

<picture data-inline-viewer="examples/rgbd">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/rgbd/4109d29ed52fa0a8f980fcdd0e9673360c76879f/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/rgbd/4109d29ed52fa0a8f980fcdd0e9673360c76879f/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/rgbd/4109d29ed52fa0a8f980fcdd0e9673360c76879f/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/rgbd/4109d29ed52fa0a8f980fcdd0e9673360c76879f/1200w.png">
  <img src="https://static.rerun.io/rgbd/4109d29ed52fa0a8f980fcdd0e9673360c76879f/full.png" alt="RGBD example screenshot">
</picture>

Example using an [example dataset](https://cs.nyu.edu/~silberman/datasets/nyu_depth_v2.html) from New York University with RGB and Depth channels.

```bash
pip install -r examples/python/rgbd/requirements.txt
python examples/python/rgbd/main.py
```
