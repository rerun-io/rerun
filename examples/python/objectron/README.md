<!--[metadata]
title = "Objectron"
tags = ["2D", "3D", "object-detection", "pinhole-camera"]
description = "Example of using the Rerun SDK to log the Google Research Objectron dataset."
thumbnail = "https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/480w.png"
thumbnail_dimensions = [480, 268]
# channel = "release"  - Disabled because it sometimes have bad first-frame heuristics
build_args = ["--frames=150"]
-->

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/1200w.png">
  <img src="https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/full.png" alt="Objectron example screenshot">
</picture>

Visualize the [Google Research Objectron](https://github.com/google-research-datasets/Objectron) dataset, which contains camera poses, sparse point-clouds and characterization of the planar surfaces in the surrounding environment.

## Used Rerun Types
[`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole), [`Points2D`](https://www.rerun.io/docs/reference/types/archetypes/points2d),  [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`Boxes3D`](https://www.rerun.io/docs/reference/types/archetypes/boxes3d), [`LineStrips2D`](https://www.rerun.io/docs/reference/types/archetypes/line_strips2d)

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
pip install -r examples/python/objectron/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/objectron/main.py # run the example
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python examples/python/objectron/main.py --help 

usage: main.py [-h] [--frames FRAMES] [--run-forever] [--per-frame-sleep PER_FRAME_SLEEP] [--recording {bike,book,bottle,camera,cereal_box,chair,cup,laptop,shoe}]
               [--force-reprocess-video] [--dataset-dir DATASET_DIR] [--headless] [--connect] [--serve] [--addr ADDR] [--save SAVE] [-o]

Logs Objectron data using the Rerun SDK.

optional arguments:
  -h, --help                             show this help message and exit
  --frames FRAMES                       If specified, limits the number of frames logged
  --run-forever                         Run forever, continually logging data.
  --per-frame-sleep PER_FRAME_SLEEP     Sleep this much for each frame read, if --run-forever
  --recording {bike,book,bottle,camera,cereal_box,chair,cup,laptop,shoe}
                                        The objectron recording to log to Rerun.
  --force-reprocess-video               Reprocess video frames even if they already exist
  --dataset-dir DATASET_DIR             Directory to save example videos to.
  --headless                            Don t show GUI
  --connect                             Connect to an external viewer
  --serve                               Serve a web viewer (WARNING: experimental feature)
  --addr ADDR                           Connect to this ip:port
  --save SAVE                           Save data to a .rrd file at this path
  -o, --stdout                          Log data to standard output, to be piped into a Rerun Viewer
```


[//]: # (Example of using the Rerun SDK to log the [Objectron]&#40;https://github.com/google-research-datasets/Objectron&#41; dataset.)

[//]: # (> The Objectron dataset is a collection of short, object-centric video clips, which are accompanied by AR session metadata that includes camera poses, sparse point-clouds and characterization of the planar surfaces in the surrounding environment.)
