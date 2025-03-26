<!--[metadata]
title = "Live depth sensor"
tags = ["2D", "3D", "Live", "Depth", "RealSense"]
thumbnail = "https://static.rerun.io/live_depth_sensor/d3c0392bebe2003d24110a779d6f6748167772d8/480w.png"
thumbnail_dimensions = [480, 360]
-->

Visualize the live-streaming frames from an Intel RealSense depth sensor.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/live_depth_sensor/d3c0392bebe2003d24110a779d6f6748167772d8/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/live_depth_sensor/d3c0392bebe2003d24110a779d6f6748167772d8/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/live_depth_sensor/d3c0392bebe2003d24110a779d6f6748167772d8/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/live_depth_sensor/d3c0392bebe2003d24110a779d6f6748167772d8/1200w.png">
  <img src="https://static.rerun.io/live_depth_sensor/d3c0392bebe2003d24110a779d6f6748167772d8/full.png" alt="Live Depth Sensor example screenshot">
</picture>

This example requires a connected realsense depth sensor.

## Used Rerun types
[`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole), [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`DepthImage`](https://www.rerun.io/docs/reference/types/archetypes/depth_image)

## Background
The Intel RealSense depth sensor can stream live depth and color data. To visualize this data output, we utilized Rerun.

## Logging and visualizing with Rerun

The RealSense sensor captures data in both RGB and depth formats, which are logged using the [`Image`](https://www.rerun.io/docs/reference/types/archetypes/image) and [`DepthImage`](https://www.rerun.io/docs/reference/types/archetypes/depth_image) archetypes, respectively.
Additionally, to provide a 3D view, the visualization includes a pinhole camera using the [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole) and [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d) archetypes.

The visualization in this example were created with the following Rerun code.

```python
rr.log("realsense", rr.ViewCoordinates.RDF, static=True) # Visualize the data as RDF
```



### Image

First, the pinhole camera is set using the [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole) and [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d) archetypes. Then, the images captured by the RealSense sensor are logged as an [`Image`](https://www.rerun.io/docs/reference/types/archetypes/image) object, and they're associated with the time they were taken.



```python
rgb_from_depth = depth_profile.get_extrinsics_to(rgb_profile)
rr.log(
    "realsense/rgb",
    rr.Transform3D(
        translation=rgb_from_depth.translation,
        mat3x3=np.reshape(rgb_from_depth.rotation, (3, 3)),
        relation=rr.TransformRelation.ChildFromParent,
    ),
    static=True,
)
```

```python
rr.log(
    "realsense/rgb/image",
    rr.Pinhole(
        resolution=[rgb_intr.width, rgb_intr.height],
        focal_length=[rgb_intr.fx, rgb_intr.fy],
        principal_point=[rgb_intr.ppx, rgb_intr.ppy],
    ),
    static=True,
)
```
```python
rr.set_time("frame_nr", sequence=frame_nr)
rr.log("realsense/rgb/image", rr.Image(color_image))
```

### Depth image

Just like the RGB images, the RealSense sensor also captures depth data. The depth images are logged as [`DepthImage`](https://www.rerun.io/docs/reference/types/archetypes/depth_image) objects and are linked with the time they were captured.

```python
rr.log(
    "realsense/depth/image",
    rr.Pinhole(
        resolution=[depth_intr.width, depth_intr.height],
        focal_length=[depth_intr.fx, depth_intr.fy],
        principal_point=[depth_intr.ppx, depth_intr.ppy],
    ),
    static=True,
)
```
```python
rr.set_time("frame_nr", sequence=frame_nr)
rr.log("realsense/depth/image", rr.DepthImage(depth_image, meter=1.0 / depth_units))
```

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
pip install -e examples/python/live_depth_sensor
```
To experiment with the provided example, simply execute the main Python script:
```bash
python -m live_depth_sensor # run the example
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python -m live_depth_sensor --help
```
