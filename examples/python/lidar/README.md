<!--[metadata]
title = "Lidar"
tags = ["lidar", "3D"]
description = "Visualize the lidar data from the nuScenes dataset."
thumbnail = "https://static.rerun.io/lidar/bcea9337044919c1524429bd26bc51a3c4db8ccb/480w.png"
thumbnail_dimensions = [480, 286]
-->


<picture>
  <img src="https://static.rerun.io/lidar/bcea9337044919c1524429bd26bc51a3c4db8ccb/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/lidar/bcea9337044919c1524429bd26bc51a3c4db8ccb/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/lidar/bcea9337044919c1524429bd26bc51a3c4db8ccb/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/lidar/bcea9337044919c1524429bd26bc51a3c4db8ccb/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/lidar/bcea9337044919c1524429bd26bc51a3c4db8ccb/1200w.png">
</picture>

Visualize the LiDAR data from the [nuScenes dataset](https://www.nuscenes.org/).

For a more extensive example including other sensors and annotations check out the [nuScenes example](https://www.rerun.io/examples/real-data/nuscenes).

## Used Rerun Types
[`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d)

# Run the Code
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
pip install -r examples/python/lidar/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/lidar/main.py # run the example
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python examples/python/lidar/main.py --help 

usage: main.py [-h] [--root-dir ROOT_DIR] [--scene-name SCENE_NAME] [--dataset-version DATASET_VERSION] [--headless] [--connect] [--serve] [--addr ADDR]
               [--save SAVE] [-o]

Visualizes lidar scans using the Rerun SDK.

optional arguments:
  -h, --help                          show this help message and exit
  --root-dir ROOT_DIR                 Root directory of nuScenes dataset
  --scene-name SCENE_NAME             Scene name to visualize (typically of form 'scene-xxxx')
  --dataset-version DATASET_VERSION   Scene id to visualize
  --headless                          Don t show GUI
  --connect                           Connect to an external viewer
  --serve                             Serve a web viewer (WARNING: experimental feature)
  --addr ADDR                         Connect to this ip:port
  --save SAVE                         Save data to a .rrd file at this path
  -o, --stdout                        Log data to standard output, to be piped into a Rerun Viewer
```