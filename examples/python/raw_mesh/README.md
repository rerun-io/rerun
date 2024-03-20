<!--[metadata]
title = "Raw Mesh"
tags = ["mesh"]
description = "Demonstrates logging of raw 3D mesh data with simple material properties."
thumbnail = "https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/480w.png"
thumbnail_dimensions = [480, 296]
channel = "release"
-->

<picture data-inline-viewer="raw_mesh">
  <img src="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/1200w.png">
</picture>

This example demonstrates how to use the Rerun SDK to log raw 3D meshes (so-called "triangle soups") and their transform hierarchy. Simple material properties are supported.

```bash
pip install -r examples/python/raw_mesh/requirements.txt
python examples/python/raw_mesh/main.py
```
