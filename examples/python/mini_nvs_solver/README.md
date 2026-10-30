<!--[metadata]
title = "Mini NVS solver"
description = "Minimal NVS Solver: a video diffusion model used as a zero-shot novel-view synthesizer along a chosen camera path."
tags = ["2D", "3D", "Hugging Face", "Depth", "Pinhole camera", "Diffusion"]
source = "https://github.com/pablovela5620/mini-nvs-solver"
thumbnail = "https://static.rerun.io/mini_nvs_solver_thumbnail/1aa75d5e3e7e8be1afb40c2b52f3a06155bd9dc1/768w.png"
thumbnail_dimensions = [768, 576]
-->


https://vimeo.com/995089150?loop=1&autopause=0&background=1&muted=1&ratio=2802:1790

## Background
Mini NVS Solver is a minimal implementation of NVS Solver. This method explores Video Diffusion Models as a Zero-Shot Novel View Synthesizer.
Given a single image, multi-image, or dynamic video and a chosen camera trajectory, NVS Solver can generate the image frames of the camera on said trajectory using stable diffusion video and the warped input views.

## Run the code
This is an external example. Check the [repository](https://github.com/pablovela5620/mini-nvs-solver) for more information.

You can try the example on Rerun's HuggingFace space [here](https://huggingface.co/spaces/pablovela5620/mini-nvs-solver)].

It is highly recommended to run this example locally by cloning the above repo and running:
```
pixi run app
```
