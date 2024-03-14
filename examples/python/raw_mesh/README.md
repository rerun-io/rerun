<!--[metadata]
title = "Raw Mesh"
tags = ["mesh"]
description = "Demonstrates logging of raw 3D mesh data with simple material properties."
thumbnail = "https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/480w.png"
thumbnail_dimensions = [480, 296]
channel = "release"
-->

<picture>
  <img src="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/1200w.png">
</picture>

Demonstrates logging of raw 3D mesh data (so-called "triangle soups") with simple material properties and their transform hierarchy.

## Used Rerun Types
[`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`Mesh3D`](https://www.rerun.io/docs/reference/types/archetypes/mesh3d)

# Run the code
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
pip install -r examples/python/raw_mesh/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/raw_mesh/main.py # run the example
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python examples/python/raw_mesh/main.py --help 

usage: main.py [-h] [--scene {lantern,avocado,buggy,brain_stem}] [--scene-path SCENE_PATH] [--headless] [--connect] [--serve] [--addr ADDR] [--save SAVE] [-o]

Logs raw 3D meshes and their transform hierarchy using the Rerun SDK.

optional arguments:
  -h, --help            show this help message and exit
  --scene {lantern,avocado,buggy,brain_stem}
                        The name of the scene to load
  --scene-path SCENE_PATH
                        Path to a scene to analyze. If set, overrides the `--scene` argument.
  --headless            Don t show GUI
  --connect             Connect to an external viewer
  --serve               Serve a web viewer (WARNING: experimental feature)
  --addr ADDR           Connect to this ip:port
  --save SAVE           Save data to a .rrd file at this path
  -o, --stdout          Log data to standard output, to be piped into a Rerun Viewer
```

[//]: # (This example demonstrates how to use the Rerun SDK to log raw 3D meshes &#40;so-called "triangle soups"&#41; and their transform hierarchy. Simple material properties are supported.)

