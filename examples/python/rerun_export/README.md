<!--[metadata]
title = "LeRobot dataset from RRD"
description = "Convert RRD robot recordings into LeRobot v3 training datasets using the OSS Rerun server, with video remuxing."
source = "https://github.com/rerun-io/rerun-lerobot"
tags = ["Robotics", "MCAP", "LeRobot", "Dataset", "Server"]
thumbnail = "https://static.rerun.io/rerun_export/f3b727db8bbe3ecf6894707ac7770d3d8fc8bf1f/480w.png"
thumbnail_dimensions = [480, 384]
-->

<picture>
  <img src="https://static.rerun.io/rerun_export/f3b727db8bbe3ecf6894707ac7770d3d8fc8bf1f/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/rerun_export/f3b727db8bbe3ecf6894707ac7770d3d8fc8bf1f/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/rerun_export/f3b727db8bbe3ecf6894707ac7770d3d8fc8bf1f/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/rerun_export/f3b727db8bbe3ecf6894707ac7770d3d8fc8bf1f/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/rerun_export/f3b727db8bbe3ecf6894707ac7770d3d8fc8bf1f/1200w.png">
</picture>

Convert robot recordings into training-ready datasets by using the OSS Rerun server to query and transform RRD files into LeRobot v3 format.

## Background

This example demonstrates how to use the Rerun OSS server API to process robot recordings and prepare them for imitation learning.
The workflow uses the server to load RRD files, query robot data (actions, observations, videos), align time series to a target frame rate, and write the result as a LeRobot v3 dataset compatible with robotics model training pipelines.

[LeRobot](https://github.com/huggingface/lerobot) is a project by Hugging Face that provides models, datasets, and tools for real-world robotics in PyTorch. This example shows how Rerun recordings can be converted into LeRobot's standardized dataset format.

## Run the code

The code lives in the [`rerun-lerobot`](https://github.com/rerun-io/rerun-lerobot) package, which you can install from PyPI:

```bash
pip install rerun-lerobot
```

You can find the source and build instructions here: [rerun-lerobot](https://github.com/rerun-io/rerun-lerobot)
