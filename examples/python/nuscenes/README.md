<!--[metadata]
title = "nuScenes"
tags = ["lidar", "3D", "2D", "object-detection", "pinhole-camera"]
description = "Visualize the nuScenes dataset including lidar, radar, images, and bounding boxes."
thumbnail = "https://static.rerun.io/nuscenes/64a50a9d67cbb69ae872551989ee807b195f6b5d/480w.png"
thumbnail_dimensions = [480, 282]
channel = "release"
build_args = ["--seconds=5"]
-->

<picture>
  <img src="https://static.rerun.io/nuscenes/64a50a9d67cbb69ae872551989ee807b195f6b5d/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/nuscenes/64a50a9d67cbb69ae872551989ee807b195f6b5d/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/nuscenes/64a50a9d67cbb69ae872551989ee807b195f6b5d/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/nuscenes/64a50a9d67cbb69ae872551989ee807b195f6b5d/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/nuscenes/64a50a9d67cbb69ae872551989ee807b195f6b5d/1200w.png">
</picture>

Visualize the [nuScenes dataset](https://www.nuscenes.org/) including lidar, radar, images, and bounding boxes data.

## Used Rerun Types
[`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`Boxes3D`](https://www.rerun.io/docs/reference/types/archetypes/boxes3d), [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole), `ImageEncoded`

# Run the Code
To run this example, make sure you have Python version at least 3.9, the Rerun repository checked out and the latest SDK installed:
```bash
# Setup 
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
pip install -r examples/python/nuscenes/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/nuscenes/main.py # run the example
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python examples/python/nuscenes/main.py --help 

usage: main.py [-h] [--root-dir ROOT_DIR] [--scene-name SCENE_NAME] [--dataset-version DATASET_VERSION] [--seconds SECONDS] [--headless] [--connect] [--serve]
               [--addr ADDR] [--save SAVE] [-o]

Visualizes the nuScenes dataset using the Rerun SDK.

optional arguments:
  -h, --help                          show this help message and exit
  --root-dir ROOT_DIR                 Root directory of nuScenes dataset
  --scene-name SCENE_NAME             Scene name to visualize (typically of form 'scene-xxxx')
  --dataset-version DATASET_VERSION   Scene id to visualize
  --seconds SECONDS                   If specified, limits the number of seconds logged
  --headless                          Don t show GUI
  --connect                           Connect to an external viewer
  --serve                             Serve a web viewer (WARNING: experimental feature)
  --addr ADDR                         Connect to this ip:port
  --save SAVE                         Save data to a .rrd file at this path
  -o, --stdout                        Log data to standard output, to be piped into a Rerun Viewer
```


[//]: # (This example visualizes the [nuScenes dataset]&#40;https://www.nuscenes.org/&#41; using Rerun. The dataset)

[//]: # (contains lidar data, radar data, color images, and labeled bounding boxes.)


