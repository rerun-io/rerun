<!--[metadata]
title = "DepthPro"
tags = ["2D", "3D", "HuggingFace", "Pinhole camera", "Depth"]
source = "https://github.com/rerun-io/hf-example-ml-depth-pro"
thumbnail = "https://static.rerun.io/ml_depth_pro/e29c5afc5e4d4a36656abe0e4559a952a5a2fa68/480w.png"
thumbnail_dimensions = [480, 294]
-->

This example visualizes the paper "Depth Pro: Sharp Monocular Metric Depth in Less Than a Second" ([arXiv](https://arxiv.org/abs/2410.02073)).
The example runs inference for each frame in the provided video, and logs the predicted depth map to Rerun.

## Background

DepthPro is a fast, zero-shot monocular depth estimation model developed by Apple.
It produces highly detailed and sharp depth maps at 2.25 megapixels in just 0.3 seconds on a standard GPU.
The model works using a multi-scale vision transformer architecture that captures both global context and fine-grained details, enabling it to
accurately predict metric depth _without_ requiring camera intrinsics such as focal length or principal point.
Additionally the model is able to predict the focal length of camera used to take the photo, which is also visualized in this example.

This example uses the open-source code and [model weights](https://huggingface.co/apple/DepthPro) provided by the authors.

## Run the code

This is an external example. Check the [repository](https://github.com/rerun-io/hf-example-ml-depth-pro) for more information.

You can try the example on a HuggingFace space [here](https://huggingface.co/spaces/oxkitsune/rerun-ml-depth-pro).
