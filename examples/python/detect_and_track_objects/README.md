---
title: Detect and Track Objects
python: https://github.com/rerun-io/rerun/tree/latest/examples/python/detect_and_track_objects/main.py
tags: [2D, huggingface, object-detection, object-tracking, opencv]
description: "Visualize object detection and segmentation using the Huggingface `transformers` library."
thumbnail: https://static.rerun.io/detect_and_track_objects/59f5b97a8724f9037353409ab3d0b7cb47d1544b/480w.png
thumbnail_dimensions: [480, 279]
channel: nightly
---

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/detect_and_track_objects/59f5b97a8724f9037353409ab3d0b7cb47d1544b/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/detect_and_track_objects/59f5b97a8724f9037353409ab3d0b7cb47d1544b/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/detect_and_track_objects/59f5b97a8724f9037353409ab3d0b7cb47d1544b/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/detect_and_track_objects/59f5b97a8724f9037353409ab3d0b7cb47d1544b/1200w.png">
  <img src="https://static.rerun.io/detect_and_track_objects/59f5b97a8724f9037353409ab3d0b7cb47d1544b/full.png" alt="">
</picture>

Another more elaborate example applying simple object detection and segmentation on a video using the Huggingface `transformers` library. Tracking across frames is performed using [CSRT](https://arxiv.org/pdf/1611.08461.pdf) from OpenCV.

For more info see [here](https://huggingface.co/docs/transformers/index)

```bash
pip install -r examples/python/detect_and_track_objects/requirements.txt
python examples/python/detect_and_track_objects/main.py
```
