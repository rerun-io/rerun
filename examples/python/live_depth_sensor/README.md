<!--[metadata]
title = "Live Depth Sensor"
tags = ["2D", "3D", "live", "depth", "realsense"]
thumbnail = "https://static.rerun.io/live_depth_sensor/d3c0392bebe2003d24110a779d6f6748167772d8/480w.png"
thumbnail_dimensions = [480, 360]
-->


<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/live_depth_sensor/d3c0392bebe2003d24110a779d6f6748167772d8/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/live_depth_sensor/d3c0392bebe2003d24110a779d6f6748167772d8/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/live_depth_sensor/d3c0392bebe2003d24110a779d6f6748167772d8/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/live_depth_sensor/d3c0392bebe2003d24110a779d6f6748167772d8/1200w.png">
  <img src="https://static.rerun.io/live_depth_sensor/d3c0392bebe2003d24110a779d6f6748167772d8/full.png" alt="Live Depth Sensor example screenshot">
</picture>

Visualize the live-streaming frames from an Intel RealSense depth sensor.

## Used Rerun Types
[`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole), [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`DepthImage`](https://www.rerun.io/docs/reference/types/archetypes/depth_image)

# Run
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
pip install -r examples/python/live_depth_sensor/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/live_depth_sensor/main.py # run the example
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python examples/python/live_depth_sensor/main.py --help 
```