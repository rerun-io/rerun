<!--[metadata]
title = "Detect and Track Objects"
tags = ["2D", "huggingface", "object-detection", "object-tracking", "opencv"]
description = "Visualize object detection and segmentation using the Huggingface `transformers` library."
thumbnail = "https://static.rerun.io/detect_and_track_objects/59f5b97a8724f9037353409ab3d0b7cb47d1544b/480w.png"
thumbnail_dimensions = [480, 279]
channel = "release"
-->


<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/detect_and_track_objects/59f5b97a8724f9037353409ab3d0b7cb47d1544b/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/detect_and_track_objects/59f5b97a8724f9037353409ab3d0b7cb47d1544b/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/detect_and_track_objects/59f5b97a8724f9037353409ab3d0b7cb47d1544b/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/detect_and_track_objects/59f5b97a8724f9037353409ab3d0b7cb47d1544b/1200w.png">
  <img src="https://static.rerun.io/detect_and_track_objects/59f5b97a8724f9037353409ab3d0b7cb47d1544b/full.png" alt="">
</picture>

Visualize object detection and segmentation using the [Huggingface's Transformers](https://huggingface.co/docs/transformers/index) and [CSRT](https://arxiv.org/pdf/1611.08461.pdf) from OpenCV.

## Used Rerun Types
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`SegmentationImage`](https://www.rerun.io/docs/reference/types/archetypes/segmentation_image), [`AnnotationContext`](https://www.rerun.io/docs/reference/types/archetypes/annotation_context), [`Boxes2D`](https://www.rerun.io/docs/reference/types/archetypes/boxes2d), [`TextLog`](https://www.rerun.io/docs/reference/types/archetypes/text_log)

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
pip install -r examples/python/detect_and_track_objects/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/detect_and_track_objects/main.py # run the example
```

If you wish to customize it for various videos, adjust the maximum frames, explore additional features, or save it use the CLI with the `--help` option for guidance:

```bash
python examples/python/detect_and_track_objects/main.py --help

usage: main.py [-h] [--video {horses, driving,boats}] [--dataset-dir DATASET_DIR] [--video-path VIDEO_PATH] [--max-frame MAX_FRAME] [--headless] [--connect]
               [--serve] [--addr ADDR] [--save SAVE] [-o]

Example applying simple object detection and tracking on a video.

optional arguments:
  -h, --help                        Show this help message and exit
  --video {horses, driving,boats}   The example video to run on.
  --dataset-dir DATASET_DIR         Directory to save example videos to.
  --video-path VIDEO_PATH           Full path to video to run on. Overrides `--video`.
  --max-frame MAX_FRAME             Stop after processing this many frames. If not specified, will run until interrupted.
  --headless                        Don t show GUI
  --connect                         Connect to an external viewer
  --serve                           Serve a web viewer (WARNING: experimental feature)
  --addr ADDR                       Connect to this ip:port
  --save SAVE                       Save data to a .rrd file at this path
  -o, --stdout                      Log data to standard output, to be piped into a Rerun Viewer
```
