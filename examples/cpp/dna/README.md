---
title: Helix
python: https://github.com/rerun-io/rerun/tree/latest/examples/python/dna/main.py
rust: https://github.com/rerun-io/rerun/tree/latest/examples/rust/dna/src/main.rs
cpp: https://github.com/rerun-io/rerun/tree/latest/examples/cpp/dna/main.cpp
tags: [3d, api-example]
description: "Simple example of logging point and line primitives to draw a 3D helix."
thumbnail: https://static.rerun.io/helix/f4c375546fa9d24f7cd3a1a715ebf75b2978817a/480w.png
thumbnail_dimensions: [480, 285]
channel: main
---

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/helix/f4c375546fa9d24f7cd3a1a715ebf75b2978817a/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/helix/f4c375546fa9d24f7cd3a1a715ebf75b2978817a/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/helix/f4c375546fa9d24f7cd3a1a715ebf75b2978817a/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/helix/f4c375546fa9d24f7cd3a1a715ebf75b2978817a/1200w.png">
  <img src="https://static.rerun.io/helix/f4c375546fa9d24f7cd3a1a715ebf75b2978817a/full.png" alt="">
</picture>

Simple example of logging point and line primitives to draw a 3D helix.


To build it from a checkout of the repository (requires a Rust toolchain):
```bash
cmake .
cmake --build . --target example_dna
./examples/cpp/dna/example_dna
```
