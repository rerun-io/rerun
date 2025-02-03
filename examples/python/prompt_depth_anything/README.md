<!--[metadata]
title = "Depth compare"
tags = ["2D", "3D", "Lidar", "Depth", "Pinhole camera"]
source = "https://github.com/rerun-io/prompt-da"
thumbnail = "https://static.rerun.io/prompt-da/c0d7f8045e5b120ec3d5fb09ce8511dd2fcb356e/480w.png"
thumbnail_dimensions = [480, 275]
-->


https://vimeo.com/1052753560?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=2802:1790

## Background
Prompt Depth Anything builds on DepthAnythingV2 by leveraging a low-resolution “prompt” depth map captured from an iPhone LiDAR along with its corresponding image to generate metric depth maps at resolutions up to 4K. This approach benefits applications that require high-resolution, metric, and multi-view consistent depth—such as 3D reconstruction and generalized robotic grasping. In this example, you can use the output from a raw Polycam scan to produce high-resolution depth maps for downstream applications.


## Run the code
This is an external example. Check the [repository](https://github.com/rerun-io/prompt-da) for more information.

You can easily run this example by doing the following (make sure you have [Pixi](https://pixi.sh/latest/#installation) installed)
```
git clone https://github.com/rerun-io/prompt-da.git
cd prompt-da
pixi run app
```
