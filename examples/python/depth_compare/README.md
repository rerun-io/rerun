<!--[metadata]
title = "Depth compare"
tags = ["2D", "3D", "HuggingFace", "Depth", "Pinhole camera"]
source = "https://github.com/pablovela5620/monoprior"
thumbnail = "https://static.rerun.io/depth-compare-thumbnail/270fdb11a2b6e85c69d83d52d475a5ec644de9df/480w.png"
thumbnail_dimensions = [480, 258]
-->


https://vimeo.com/969623509?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=2802:1790

## Background
Depth Compare allows for easy comparison between different depth models, both metric and scale + shift invariant. There has been a recent flurry of monocular depth estimation networks, and often the only method of comparison is a 2D depth image. This does not provide the full scope of how well a depth network performs in reality. Depth Compare allows for 3D back-projection of the depth image into world space, along with outputting the point cloud to give a better understanding.

## Run the code
This is an external example. Check the [repository](https://github.com/pablovela5620/monoprior) for more information.

You can try the example on Rerun's HuggingFace space [here](https://huggingface.co/spaces/pablovela5620/depth-compare).

You can also run things locally by cloning the above repo and running:
```
pixi run app
```
