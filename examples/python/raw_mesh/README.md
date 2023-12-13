---
title: Raw Mesh
python: https://github.com/rerun-io/rerun/tree/latest/examples/python/raw_mesh/main.py
rust: https://github.com/rerun-io/rerun/tree/latest/examples/rust/raw_mesh/src/main.rs
tags: [mesh]
thumbnail: https://static.rerun.io/raw_mesh/64bec98280b07794f7c9617f30ba2c20278601c3/480w.png
thumbnail_dimensions: [480, 271]
demo: true
nightly: true
---

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/raw_mesh/64bec98280b07794f7c9617f30ba2c20278601c3/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/raw_mesh/64bec98280b07794f7c9617f30ba2c20278601c3/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/raw_mesh/64bec98280b07794f7c9617f30ba2c20278601c3/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/raw_mesh/64bec98280b07794f7c9617f30ba2c20278601c3/1200w.png">
  <img src="https://static.rerun.io/raw_mesh/64bec98280b07794f7c9617f30ba2c20278601c3/full.png" alt="Raw Mesh example screenshot">
</picture>

This example demonstrates how to use the Rerun SDK to log raw 3D meshes (so-called "triangle soups") and their transform hierarchy. Simple material properties are supported.

```bash
pip install -r examples/python/raw_mesh/requirements.txt
python examples/python/raw_mesh/main.py
```
