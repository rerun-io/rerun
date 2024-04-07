<!--[metadata]
title = "Structure from motion"
tags = ["2D", "3D", "COLMAP", "Pinhole camera", "Time series"]
thumbnail = "https://static.rerun.io/structure-from-motion/af24e5e8961f46a9c10399dbc31b6611eea563b4/480w.png"
thumbnail_dimensions = [480, 480]
channel = "main"
build_args = ["--dataset=colmap_fiat", "--resize=800x600"]
-->

Visualize a sparse reconstruction by [COLMAP](https://colmap.github.io/index.html), a general-purpose Structure-from-Motion (SfM) and Multi-View Stereo (MVS) pipeline with a graphical and command-line interface

<picture data-inline-viewer="examples/structure_from_motion">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/structure_from_motion/b17f8824291fa1102a4dc2184d13c91f92d2279c/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/structure_from_motion/b17f8824291fa1102a4dc2184d13c91f92d2279c/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/structure_from_motion/b17f8824291fa1102a4dc2184d13c91f92d2279c/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/structure_from_motion/b17f8824291fa1102a4dc2184d13c91f92d2279c/1200w.png">
  <img src="https://static.rerun.io/structure_from_motion/b17f8824291fa1102a4dc2184d13c91f92d2279c/full.png" alt="Structure From Motion example screenshot">
</picture>

In this example a short video clip has been processed offline by the COLMAP pipeline, and we use Rerun to visualize the individual camera frames, estimated camera poses, and resulting point clouds over time.


```bash
pip install -r examples/python/structure_from_motion/requirements.txt
python examples/python/structure_from_motion/main.py
```
