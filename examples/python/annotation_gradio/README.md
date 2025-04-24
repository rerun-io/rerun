<!--[metadata]
title = "Interactive 3D Annotation App with Rerun and Gradio"
tags = ["2D", "3D", "Pinhole camera", "Time series", "SAM", "Segmentation"]
source = "https://github.com/rerun-io/annotation-example"
thumbnail = "https://static.rerun.io/square-thumbnail/56b091d93d23c4353f6a919bf789493da19893e6/480w.png"
thumbnail_dimensions = [480, 474]
-->

https://vimeo.com/1078165216?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=2802:1790

## Background

This example showcases how to use Rerun with gradio to generate an annotation app. It consists of two different modes both of which leverage Segment Anything 2.

The first mode focuses on tracking an object in a monocular video stream. In addition to segmentation masks, it generates real-time depth maps and point clouds to provide full 3D spatial context, enabling users to visualize, inspect, and annotate the tracked object directly in three-dimensional space.

The second mode uses a multiview RGB-D video dataset. By obtaining segmentation masks from two synchronized and calibrated RGB-D views, the app triangulates these 2D masks to reconstruct a precise 3D mask of the chosen object. It then propagates this 3D mask across all camera views and through each frame of the videos, resulting in a fully tracked 3D object trajectory over time.

## Run the code

This is an external example. Check the [repository](https://github.com/rerun-io/annotation-example) for more information on how to run the code.

TLDR: make sure you have the [Pixi package manager](https://pixi.sh/latest/#installation) installed and run
```
git clone https://github.com/rerun-io/annotation-example
cd annotation-example
pixi run app
```

this will run the single view (monocular) app

```
pixi run multiview-app
```
will run the multiview rgb-d app
