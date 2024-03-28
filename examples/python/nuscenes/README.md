<!--[metadata]
title = "nuScenes"
tags = ["lidar", "3D", "2D", "object-detection", "pinhole-camera"]
description = "Visualize the nuScenes dataset including lidar, radar, images, and bounding boxes."
thumbnail = "https://static.rerun.io/nuscenes/64a50a9d67cbb69ae872551989ee807b195f6b5d/480w.png"
thumbnail_dimensions = [480, 282]
channel = "release"
build_args = ["--seconds=5"]
-->

<picture data-inline-viewer="examples/nuscenes">
  <img src="https://static.rerun.io/nuscenes/64a50a9d67cbb69ae872551989ee807b195f6b5d/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/nuscenes/64a50a9d67cbb69ae872551989ee807b195f6b5d/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/nuscenes/64a50a9d67cbb69ae872551989ee807b195f6b5d/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/nuscenes/64a50a9d67cbb69ae872551989ee807b195f6b5d/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/nuscenes/64a50a9d67cbb69ae872551989ee807b195f6b5d/1200w.png">
</picture>

Visualize the [nuScenes dataset](https://www.nuscenes.org/) including lidar, radar, images, and bounding boxes data.

# Used Rerun Types
[`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`Boxes3D`](https://www.rerun.io/docs/reference/types/archetypes/boxes3d), [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole), [`Image`](https://ref.rerun.io/docs/python/0.14.1/common/image_helpers/#rerun.ImageEncoded)<sup>*</sup>

# Background
This example demonstrates the ability to read and visualize scenes from the nuScenes dataset, which is a public large-scale dataset specifically designed for autonomous driving. 
The scenes in this dataset encompass data collected from a comprehensive suite of sensors on autonomous vehicles. 
These include 6 cameras, 1 LIDAR, 5 RADAR, GPS and IMU sensors. 
Consequently, the dataset provides information about the vehicle's pose, the images captured, the recorded sensor data and the results of object detection at any given moment.


# Logging and Visualizing with Rerun

The visualizations in this example were created with the following Rerun code:

## Sensor Calibration

First, pinhole cameras and sensor poses are initialized to offer a 3D view and camera perspective. This is achieved using the [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole) and [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d) archetypes.

```python
rr.log(
        f"world/ego_vehicle/{sensor_name}",
        rr.Transform3D(
            translation=calibrated_sensor["translation"],
            rotation=rr.Quaternion(xyzw=rotation_xyzw),
            from_parent=False,
        ),
        timeless=True,
    )
```

```python
rr.log(
            f"world/ego_vehicle/{sensor_name}",
            rr.Pinhole(
                image_from_camera=calibrated_sensor["camera_intrinsic"],
                width=sample_data["width"],
                height=sample_data["height"],
            ),
            timeless=True,
        )
```

## Timelines

All data logged using Rerun in the following sections is initially connected to a specific time.
Rerun assigns a timestamp to each piece of logged data, and these timestamps are associated with [`timelines`](https://www.rerun.io/docs/concepts/timelines).

```python
rr.set_time_seconds("timestamp", sample_data["timestamp"] * 1e-6)
```


## Vehicle Pose

As the vehicle is moving, its pose needs to be updated. Consequently, the positions of pinhole cameras and sensors must also be adjusted using [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d).
```python
rr.log(
    "world/ego_vehicle",
    rr.Transform3D(
        translation=ego_pose["translation"],
        rotation=rr.Quaternion(xyzw=rotation_xyzw),
        from_parent=False,
    ),
)
```

## LiDAR Data
LiDAR data is logged as [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d) archetype.
```python
rr.log(f"world/ego_vehicle/{sensor_name}", rr.Points3D(points, colors=point_colors))
```

## Camera Data
Camera data is logged as encoded images using [`ImageEncoded`](https://ref.rerun.io/docs/python/0.14.1/common/image_helpers/#rerun.ImageEncoded).
```python
rr.log(f"world/ego_vehicle/{sensor_name}", rr.ImageEncoded(path=data_file_path))
```

## Radar Data
Radar data is logged similar to LiDAR data, as [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d).
```python
rr.log(f"world/ego_vehicle/{sensor_name}", rr.Points3D(points, colors=point_colors))
```

## Annotations

Annotations are logged as [`Boxes3D`](https://www.rerun.io/docs/reference/types/archetypes/boxes3d), containing details such as object positions, sizes, and rotation.
```python
rr.log("world/anns", rr.Boxes3D(sizes=sizes, centers=centers, rotations=rotations, class_ids=class_ids))
```


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
```
