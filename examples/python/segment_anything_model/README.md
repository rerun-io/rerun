<!--[metadata]
title = "Segment Anything Model"
tags = ["2D", "sam", "segmentation"]
description = "Example of using Rerun to log and visualize the output of Meta AI's Segment Anything model."
thumbnail = "https://static.rerun.io/segment_anything_model/6aa2651907efbcf81be55b343caa76b9de5f2138/480w.png"
thumbnail_dimensions = [480, 283]
channel = "release"
-->


<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/segment_anything_model/6aa2651907efbcf81be55b343caa76b9de5f2138/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/segment_anything_model/6aa2651907efbcf81be55b343caa76b9de5f2138/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/segment_anything_model/6aa2651907efbcf81be55b343caa76b9de5f2138/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/segment_anything_model/6aa2651907efbcf81be55b343caa76b9de5f2138/1200w.png">
  <img src="https://static.rerun.io/segment_anything_model/6aa2651907efbcf81be55b343caa76b9de5f2138/full.png" alt="Segment Anything Model example screenshot">
</picture>

Visualize the output of [Meta AI's Segment Anything model](https://segment-anything.com/).

## Used Rerun Types
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole), [`DepthImage`](https://www.rerun.io/docs/reference/types/archetypes/depth_image)


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
pip install -r examples/python/segment_anything_model/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/segment_anything_model/main.py # run the example
```
If you wish to customize it or explore additional features, use the CLI with the `--help` option for guidance:
```bash
python examples/python/segment_anything_model/main.py --help 

usage: main.py [-h] [--model {vit_h,vit_l,vit_b}] [--device DEVICE] [--points-per-batch POINTS_PER_BATCH] [--headless] [--connect] [--serve] [--addr ADDR]
               [--save SAVE] [-o]
               [N ...]

Run the Facebook Research Segment Anything example.

positional arguments: N                   A list of images to process. (default: None)

optional arguments:
  -h, --help                              show this help message and exit
  --model {vit_h,vit_l,vit_b}             Which model to use.(See: https://github.com/facebookresearch/segment-anything#model-checkpoints) (default: vit_b)
  --device DEVICE                         Which torch device to use, e.g. cpu or cuda. (See: https://pytorch.org/docs/stable/tensor_attributes.html#torch.device) (default: cpu)
  --points-per-batch POINTS_PER_BATCH     Points per batch. More points will run faster, but too many will exhaust GPU memory. (default: 32)
  --headless                              Don t show GUI (default: False)
  --connect                               Connect to an external viewer (default: False)
  --serve                                 Serve a web viewer (WARNING: experimental feature) (default: False)
  --addr ADDR                             Connect to this ip:port (default: None)
  --save SAVE                             Save data to a .rrd file at this path (default: None)
  -o, --stdout                            Log data to standard output, to be piped into a Rerun Viewer (default: False)
```