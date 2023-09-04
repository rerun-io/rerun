---
title: Detect and Track Objects
python: https://github.com/rerun-io/rerun/tree/latest/examples/python/detect_and_track_objects/main.py
tags: [2D, huggingface, object-detection, object-tracking, opencv]
thumbnail: https://static.rerun.io/efb301d64eef6f25e8f6ae29294bd003c0cda3a7_detect_and_track_objects_480w.png
---

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/efb301d64eef6f25e8f6ae29294bd003c0cda3a7_detect_and_track_objects_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/a3df0cb3670a9f60fe0faf47ecec8d07433e1c0f_detect_and_track_objects_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/0f88a4c52aa3f3bafd42063208f10f070383380c_detect_and_track_objects_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/b4b918d8247ba2bb43c51cd2141e1e21de990e51_detect_and_track_objects_1200w.png">
  <img src="https://static.rerun.io/59f5b97a8724f9037353409ab3d0b7cb47d1544b_detect_and_track_objects_full.png" alt="">
</picture>

Another more elaborate example applying simple object detection and segmentation on a video using the Huggingface `transformers` library. Tracking across frames is performed using [CSRT](https://arxiv.org/pdf/1611.08461.pdf) from OpenCV.

For more info see [here](https://huggingface.co/docs/transformers/index)

```bash
pip install -r examples/python/detect_and_track_objects/requirements.txt
python examples/python/detect_and_track_objects/main.py
```
