<!--[metadata]
title = "ARKit Scenes"
tags = ["2D", "3D", "depth", "mesh", "object-detection", "pinhole-camera"]
description = "Visualize the ARKitScenes dataset, which contains color+depth images, the reconstructed mesh and labeled bounding boxes."
thumbnail = "https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/480w.png"
thumbnail_dimensions = [480, 243]
channel = "main"
-->


<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/1200w.png">
  <img src="https://static.rerun.io/arkit_scenes/fb9ec9e8d965369d39d51b17fc7fc5bae6be10cc/full.png" alt="ARKit Scenes screenshot">
</picture>

Visualize 3D indoor scenes using the [ARKitScenes](https://github.com/apple/ARKitScenes/) dataset, which contains RGB-D images, the reconstructed mesh and labeled bounding boxes around furniture.


## Used Rerun Types
[`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole), [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`DepthImage`](https://www.rerun.io/docs/reference/types/archetypes/depth_image), [`Mesh3D`](https://www.rerun.io/docs/reference/types/archetypes/mesh3d), [`Boxes3D`](https://www.rerun.io/docs/reference/types/archetypes/boxes3d)

## Background

ARKitScenes is a dataset provided by Apple that contains mobile RGB-D data captured using Apple's ARKit framework, which enables developers to create augmented reality experiences. 
The dataset contains color images, depth images, the reconstructed mesh, and labeled bounding boxes around furniture.

[//]: # (Immersive visualizations can be created to provide insights into indoor environments captured using mobile devices. )

## Run the Code

To run this example, make sure you have the Rerun repository checked out and the latest SDK installed:
```bash
# Setup
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```

Install the necessary libraries specified in the requirements file:
```bash
pip install -r examples/python/arkit_scenes/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/arkit_scenes/main.py # run the example
```

If you wish to customize it or explore additional features, use the CLI with the `--help` option for guidance:

```bash
python examples/python/arkit_scenes/main.py --help

usage: main.py [-h] [--video-id {48458663,42444949,41069046,41125722,41125763,42446167}] [--include-highres] [--headless] [--connect] [--serve] [--addr ADDR]
               [--save SAVE] [-o]

Visualizes the ARKitScenes dataset using the Rerun SDK.

optional arguments:
  -h, --help            show this help message and exit
  --video-id {48458663,42444949,41069046,41125722,41125763,42446167}
                        Video ID of the ARKitScenes Dataset
  --include-highres     Include the high resolution camera and depth images
  --headless            Don t show GUI
  --connect             Connect to an external viewer
  --serve               Serve a web viewer (WARNING: experimental feature)
  --addr ADDR           Connect to this ip:port
  --save SAVE           Save data to a .rrd file at this path
  -o, --stdout          Log data to standard output, to be piped into a Rerun Viewer


```


[//]: # (Use real-world RGB-D dataset &#40;[ARKitScenes]&#40;https://github.com/apple/ARKitScenes/&#41;&#41;, to visualise 3D indoor scenes using Rerun.)
[//]: # (Use the [ARKitScenes]&#40;https://github.com/apple/ARKitScenese&#41; dataset to visualise 3D indoor scenes using Rerun.)

[//]: # (the [ARKitScenes dataset]&#40;https://github.com/apple/ARKitScenes/&#41;, real-world datasets, to visualize 3D indoor scenes)

[//]: # ()
[//]: # (Visualizes the [ARKitScenes dataset]&#40;https://github.com/apple/ARKitScenes/&#41; using Rerun. )



[//]: # (This example visualizes the [ARKitScenes dataset]&#40;https://github.com/apple/ARKitScenes/&#41; using Rerun. The dataset)

[//]: # (contains color images, depth images, the reconstructed mesh, and labeled bounding boxes around furniture.)

[//]: # ()

[//]: # (The [MediaPipe Pose Landmark Detection]&#40;https://developers.google.com/mediapipe/solutions/vision/pose_landmarker&#41; solution detects and tracks human pose landmarks and produces segmentation masks for humans. The solution targets real-time inference on video streams. In this example we use Rerun to visualize the output of the Mediapipe solution over time to make it easy to analyze the behavior.)


[//]: # (```bash)

[//]: # (pip install -r examples/python/arkit_scenes/requirements.txt)

[//]: # (python examples/python/arkit_scenes/main.py)

[//]: # (```)
