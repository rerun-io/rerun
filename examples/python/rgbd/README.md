<!--[metadata]
title = "RGBD"
tags = ["2D", "3D", "depth", "nyud", "pinhole-camera"]
description = "Visualizes an example recording from the NYUD dataset with RGB and Depth channels."
thumbnail = "https://static.rerun.io/rgbd/4109d29ed52fa0a8f980fcdd0e9673360c76879f/480w.png"
thumbnail_dimensions = [480, 254]
channel = "release"
build_args = ["--frames=300"]
-->

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/rgbd/4109d29ed52fa0a8f980fcdd0e9673360c76879f/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/rgbd/4109d29ed52fa0a8f980fcdd0e9673360c76879f/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/rgbd/4109d29ed52fa0a8f980fcdd0e9673360c76879f/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/rgbd/4109d29ed52fa0a8f980fcdd0e9673360c76879f/1200w.png">
  <img src="https://static.rerun.io/rgbd/4109d29ed52fa0a8f980fcdd0e9673360c76879f/full.png" alt="RGBD example screenshot">
</picture>

Visualizes an [example dataset](https://cs.nyu.edu/~silberman/datasets/nyu_depth_v2.html) from the New York University with RGB and Depth channels.

## Used Rerun Types
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole), [`DepthImage`](https://www.rerun.io/docs/reference/types/archetypes/depth_image)

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
pip install -r examples/python/rgbd/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/rgbd/main.py # run the example
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python examples/python/rgbd/main.py --help 

usage: main.py [-h] [--recording {cafe,basements,studies,office_kitchens,playroooms}] [--subset-idx SUBSET_IDX] [--frames FRAMES] [--headless] [--connect] [--serve]
               [--addr ADDR] [--save SAVE] [-o]

Example using an example depth dataset from NYU.

optional arguments:
  -h, --help            show this help message and exit
  --recording {cafe,basements,studies,office_kitchens,playroooms}
                        Name of the NYU Depth Dataset V2 recording
  --subset-idx SUBSET_IDX
                        The index of the subset of the recording to use.
  --frames FRAMES       If specified, limits the number of frames logged
  --headless            Don t show GUI
  --connect             Connect to an external viewer
  --serve               Serve a web viewer (WARNING: experimental feature)
  --addr ADDR           Connect to this ip:port
  --save SAVE           Save data to a .rrd file at this path
  -o, --stdout          Log data to standard output, to be piped into a Rerun Viewer
```

[//]: # (Example using an [example dataset]&#40;https://cs.nyu.edu/~silberman/datasets/nyu_depth_v2.html&#41; from New York University with RGB and Depth channels.)
