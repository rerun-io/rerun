---
title: Objectron
python: https://github.com/rerun-io/rerun/tree/latest/examples/python/objectron/main.py
rust: https://github.com/rerun-io/rerun/tree/latest/examples/rust/objectron/src/main.rs
tags: [2D, 3D, object-detection, pinhole-camera]
---

![objectron example>](https://static.rerun.io/110824b31a3fe4e23b481d5fe3ed9fef2306027e_objectron1.png)

Example of using the Rerun SDK to log the [Objectron](https://github.com/google-research-datasets/Objectron) dataset.

> The Objectron dataset is a collection of short, object-centric video clips, which are accompanied by AR session metadata that includes camera poses, sparse point-clouds and characterization of the planar surfaces in the surrounding environment.

```bash
pip install -r examples/python/objectron/requirements.txt
python examples/python/objectron/main.py
```
