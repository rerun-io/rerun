---
title: Detect and Track Objects
python: https://github.com/rerun-io/rerun/tree/latest/examples/python/detect_and_track_objects/main.py
tags: [2D, huggingface, object-detection, object-tracking, opencv]
thumbnail: https://static.rerun.io/04a244d056f9cfb2ac496830392916d613902def_detect_and_track_objects_480w.png
---

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/04a244d056f9cfb2ac496830392916d613902def_detect_and_track_objects_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/d9b970d5388bcbaa631b20938a941a19e47f316d_detect_and_track_objects_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/3c8f6c4a24ed89f8cad351c25b7b39affa9d48a4_detect_and_track_objects_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/ddfe03d04002ad9ee1b545cdad9eb02eb1e35d9f_detect_and_track_objects_1200w.png">
  <img src="https://static.rerun.io/27aa9cc1ff7ae05f45193ce6d38dd1ed60f70276_detect_and_track_objects_full.png" alt="Detect and Track Objects example screenshot">
</picture>

Another more elaborate example applying simple object detection and segmentation on a video using the Huggingface `transformers` library. Tracking across frames is performed using [CSRT](https://arxiv.org/pdf/1611.08461.pdf) from OpenCV.

For more info see [here](https://huggingface.co/docs/transformers/index)

```bash
pip install -r examples/python/detect_and_track_objects/requirements.txt
python examples/python/detect_and_track_objects/main.py
```
