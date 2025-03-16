<!--[metadata]
title = "Lidar"
tags = ["Lidar", "3D"]
thumbnail = "https://static.rerun.io/lidar/caaf3b9531e50285442d17f0bc925eb7c8e12246/480w.png"
thumbnail_dimensions = [480, 480]
-->

Visualize the LiDAR data from the [nuScenes dataset](https://www.nuscenes.org/).

<picture>
  <img src="https://static.rerun.io/lidar/bcea9337044919c1524429bd26bc51a3c4db8ccb/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/lidar/bcea9337044919c1524429bd26bc51a3c4db8ccb/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/lidar/bcea9337044919c1524429bd26bc51a3c4db8ccb/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/lidar/bcea9337044919c1524429bd26bc51a3c4db8ccb/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/lidar/bcea9337044919c1524429bd26bc51a3c4db8ccb/1200w.png">
</picture>

## Used Rerun types
[`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d)

## Background
This example demonstrates the ability to read and visualize LiDAR data from the nuScenes dataset, which is a public large-scale dataset specifically designed for autonomous driving.
The scenes in this dataset encompass data collected from a comprehensive suite of sensors on autonomous vehicles, including 6 cameras, 1 LIDAR, 5 RADAR, GPS and IMU sensors.


It's important to note that in this example, only the LiDAR data is visualized. For a more extensive example including other sensors and annotations check out the [nuScenes example](https://www.rerun.io/examples/robotics/nuscenes_dataset).

## Logging and visualizing with Rerun

The visualization in this example was created with just the following lines.


```python
rr.set_time("timestamp", datetime=sample_data["timestamp"] * 1e-6) # Setting the time
rr.log("world/lidar", rr.Points3D(points, colors=point_colors)) # Log the 3D data
```

When logging data to Rerun, it's possible to associate it with specific time by using the Rerun's [`timelines`](https://www.rerun.io/docs/concepts/timelines).
In the following code, we first establish the desired time frame and then proceed to log the 3D data points.

## Run the code
To run this example, make sure you have the Rerun repository checked out and the latest SDK installed:
```bash
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
pip install -e examples/python/lidar
```
To experiment with the provided example, simply execute the main Python script:
```bash
python -m lidar # run the example
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python -m lidar --help
```
