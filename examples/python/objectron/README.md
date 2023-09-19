---
title: Objectron
python: https://github.com/rerun-io/rerun/tree/latest/examples/python/objectron/main.py
rust: https://github.com/rerun-io/rerun/tree/latest/examples/rust/objectron/src/main.rs
tags: [2D, 3D, object-detection, pinhole-camera]
thumbnail: https://static.rerun.io/d218170b8f4bfbc38ea5918747e595cff841029e_objectron_480w.png
thumbnail_dimensions: [480, 268]
---

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/d218170b8f4bfbc38ea5918747e595cff841029e_objectron_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/9a55fb96ef37bbecd2267a452cbebd85ca44d929_objectron_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/cc35c9685950c8201408a6588eac751df1de2d05_objectron_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/70ebd9b005d5ede8fa70ddab2d0b9d0c28c103ea_objectron_1200w.png">
  <img src="https://static.rerun.io/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8_objectron_full.png" alt="Objectron example screenshot">
</picture>

Example of using the Rerun SDK to log the [Objectron](https://github.com/google-research-datasets/Objectron) dataset.

> The Objectron dataset is a collection of short, object-centric video clips, which are accompanied by AR session metadata that includes camera poses, sparse point-clouds and characterization of the planar surfaces in the surrounding environment.

```bash
pip install -r examples/python/objectron/requirements.txt
python examples/python/objectron/main.py
```
