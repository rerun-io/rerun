---
title: Detect and Track Objects
python: https://github.com/rerun-io/rerun/tree/latest/examples/python/tracking_hf_opencv/main.py
tags: 2D, huggingface, object-detection, object-tracking, opencv
---

![tracking_hf_opencv example>](https://static.rerun.io/4995d2ec51249accbd287fdaef5debbfe9645a83_tracking_hf_opencv1.png)

Another more elaborate example applying simple object detection and segmentation on a video using the Huggingface `transformers` library. Tracking across frames is performed using [CSRT](https://arxiv.org/pdf/1611.08461.pdf) from OpenCV.

For more info see [here](https://huggingface.co/docs/transformers/index)

```bash
pip install -r examples/python/tracking_hf_opencv/requirements.txt
python examples/python/tracking_hf_opencv/main.py
```
