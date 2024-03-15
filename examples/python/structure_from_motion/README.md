<!--[metadata]
title = "Structure from Motion"
tags = ["2D", "3D", "colmap", "pinhole-camera", "time-series"]
description = "Visualize a sparse reconstruction by COLMAP, a general-purpose Structure-from-Motion and Multi-View Stereo pipeline."
thumbnail = "https://static.rerun.io/structure_from_motion/b17f8824291fa1102a4dc2184d13c91f92d2279c/480w.png"
thumbnail_dimensions = [480, 275]
channel = "main"
build_args = ["--dataset=colmap_fiat", "--resize=800x600"]
-->


<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/structure_from_motion/b17f8824291fa1102a4dc2184d13c91f92d2279c/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/structure_from_motion/b17f8824291fa1102a4dc2184d13c91f92d2279c/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/structure_from_motion/b17f8824291fa1102a4dc2184d13c91f92d2279c/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/structure_from_motion/b17f8824291fa1102a4dc2184d13c91f92d2279c/1200w.png">
  <img src="https://static.rerun.io/structure_from_motion/b17f8824291fa1102a4dc2184d13c91f92d2279c/full.png" alt="Structure From Motion example screenshot">
</picture>

Visualize a sparse reconstruction by COLMAP, a general-purpose Structure-from-Motion and Multi-View Stereo pipeline.

A short video clip has been processed offline by the COLMAP pipeline, and we use Rerun to visualize the individual camera frames, estimated camera poses, and resulting point clouds over time.

[//]: # (An example using Rerun to log and visualize the output of COLMAP's sparse reconstruction.)

## Background

[COLMAP](https://colmap.github.io/index.html) is a general-purpose Structure-from-Motion (SfM) and Multi-View Stereo (MVS) pipeline with a graphical and command-line interface.


## Used Rerun Types

[`Points2D`](https://www.rerun.io/docs/reference/types/archetypes/points2d), [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`SeriesLine`](https://www.rerun.io/docs/reference/types/archetypes/series_line), [`Scalar`](https://www.rerun.io/docs/reference/types/archetypes/scalar), [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole), [`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`TextDocument`](https://www.rerun.io/docs/reference/types/archetypes/text_document)

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
pip install -r examples/python/structure_from_motion/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/structure_from_motion/main.py # run the example
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python examples/python/structure_from_motion/main.py --help 

usage: main.py [-h] [--unfiltered] [--dataset {colmap_rusty_car,colmap_fiat}] [--resize RESIZE] [--headless] [--connect] [--serve] [--addr ADDR] [--save SAVE] [-o]

Visualize the output of COLMAP s sparse reconstruction on a video.

optional arguments:
  -h, --help                                    show this help message and exit
  --unfiltered                                  If set, we don t filter away any noisy data.
  --dataset {colmap_rusty_car,colmap_fiat}      Which dataset to download
  --resize RESIZE                               Target resolution to resize images
  --headless                                    Don t show GUI
  --connect                                     Connect to an external viewer
  --serve                                       Serve a web viewer (WARNING: experimental feature)
  --addr ADDR                                   Connect to this ip:port
  --save SAVE                                   Save data to a .rrd file at this path
  -o, --stdout                                  Log data to standard output, to be piped into a Rerun Viewer
```