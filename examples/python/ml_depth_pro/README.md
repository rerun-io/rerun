<!--[metadata]
title = "Depth Pro"
tags = ["2D", "3D", "HuggingFace", "Pinhole camera", "Depth"]
source = "https://github.com/rerun-io/hf-example-ml-depth-pro"
thumbnail = "https://static.rerun.io/ml_depth_pro/e29c5afc5e4d4a36656abe0e4559a952a5a2fa68/480w.png"
thumbnail_dimensions = [480, 294]
-->

This example visualizes the paper "Depth Pro: Sharp Monocular Metric Depth in Less Than a Second". The example runs
inference for each frame in the provided video, and logs the predicted depth map to Rerun.

## Background

[DepthPro](https://huggingface.co/apple/DepthPro) is a fast metric depth prediction model by Apple.
The model synthesizes high-resolution depth maps with unparalleled sharpness and high-frequency details. The predictions are metric, with absolute scale, without relying on the availability of metadata such as camera intrinsics. And the model is fast, producing a 2.25-megapixel depth map in 0.3 seconds on a standard GPU.

## Run the code

This is an external example. Check the [repository](https://github.com/rerun-io/hf-example-ml-depth-pro) for more information.

You can try the example on a HuggingFace space [here](https://huggingface.co/spaces/oxkitsune/rerun-ml-depth-pro).


