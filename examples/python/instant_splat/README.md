<!--[metadata]
title = "InstantSplat"
tags = ["2D", "3D", "HuggingFace", "Pinhole camera", "Point cloud"]
source = "https://github.com/pablovela5620/InstantSplat"
thumbnail = "https://static.rerun.io/final_instantsplat/e488f427179b0f439e60b6a0c29440fc836860dd/480w.png"
thumbnail_dimensions = [480, 275]
-->


https://vimeo.com/1019845514?loop=1&autopause=0&background=1&muted=1&ratio=2802:1790

## Background
InstantSplat is a sparse-view, SfM-free framework for large-scale scene reconstruction method using Gaussian Splatting. It allows for extremely fast reconstruction by using Dust3r, a multiview stereo network, to initialize camera poses and dense point cloud for all training views. To enhance pose accuracy and tune scene parameters a gradient-based joint optimization framework from self-supervision is used. By employing this simplified framework, InstantSplat achieves a substantial reduction in training time, from hours to mere seconds, and demonstrates robust performance across various numbers of views in diverse datasets

## Run the code
This is an external example. Check the [repository](https://github.com/pablovela5620/InstantSplat) for more information.

You can try the example on HuggingFace space [here](https://huggingface.co/spaces/pablovela5620/instant-splat).

It is highly recommended to run this example locally by cloning the above repo and running (make sure you have [Pixi](https://pixi.sh/latest/#installation) installed):
```
git clone https://github.com/pablovela5620/InstantSplat.git
pixi run app
```
