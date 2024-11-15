<!--[metadata]
title = "Sam2 + DepthAnything2"
tags = ["2D", "3D", "HuggingFace", "Depth", "Pinhole camera", "SAM", "Segmentation"]
source = "https://github.com/pablovela5620/sam2-depthanything"
thumbnail = "https://static.rerun.io/sam2_depthanything/ecc229c54a04c55bfba236f86e15cd285429e412/480w.png"
thumbnail_dimensions = [480, 268]
-->


https://vimeo.com/1003789426?loop=1&autopause=0&background=1&muted=1&ratio=2802:1790

## Background
Segment Anything 2 is follow up work on Segment Anything, that extends the state of the art segmentation capabilities into videos. This is done by adding a per session memory module that captures information about the target object in the video. This allows SAM 2 to track the selected object throughout all video frames, even if the object temporarily disappears from view, as the model has context of the object from previous frames. Depth Anything 2 is a monocular depth estimation model trained on a large amount of synthetic data and real data to achieve state of the art depth estimation. The two models are combined to allow tracking an object in 3D from just a single monocular video!

## Run the code
This is an external example. Check the [repository](https://github.com/pablovela5620/sam2-depthanything) for more information.

You can try the example on HuggingFace space [here](https://huggingface.co/spaces/pablovela5620/sam2-depthanything).

It is highly recommended to run this example locally by cloning the above repo and running (make sure you have [Pixi](https://pixi.sh/latest/#installation) installed):
```
git clone https://github.com/pablovela5620/sam2-depthanything.git
pixi run app
```
